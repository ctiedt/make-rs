//! A subset of the `make` utility.

/// A [Makefile] is represented as a list of [Target]s.
#[derive(Debug)]
struct Makefile {
    targets: Vec<Target>,
}

/// A Target's dependency. Can be another [Target] or a file.
enum Dependency<'a> {
    Target(&'a Target),
    File(&'a str),
}

/// Domain-specific errors that can happen when
/// parsing or executing a Makefile.
#[derive(Debug)]
enum MakeError {
    DependencyDoesNotExist,
    NoTargets,
    LineIsNotATarget,
    BuildError,
    NoSuchTarget,
}

impl std::fmt::Display for MakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for MakeError {}

/// A single make target with a name,
/// dependencies and a list of commands.
/// Dependencies are strings because graphs
/// are difficult in Rust.
#[derive(Debug)]
struct Target {
    name: String,
    dependencies: Vec<String>,
    commands: Vec<String>,
}

impl Target {
    /// Build this target. Assumes that dependencies
    /// have already been built and are valid.
    fn make(&self) -> Result<(), Box<dyn std::error::Error>> {
        for command in &self.commands {
            println!("{}", command);

            // Execute the command in a shell process.
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                eprint!("{}", stderr);
                return Err(Box::new(MakeError::BuildError));
            }
        }

        Ok(())
    }
}

impl Makefile {
    /// Parse a Makefile from a string.
    fn from_str<T: AsRef<str>>(data: T) -> Result<Self, Box<dyn std::error::Error>> {
        let mut targets = Vec::new();

        // First, we split the input into lines
        // and filter out the empty ones and comments.
        // We also filter out inline comments.
        let mut lines = data
            .as_ref()
            .lines()
            .filter(|line| !(line.is_empty() || line.trim().starts_with('#')))
            .map(|line| {
                if let Some((ln, _comment)) = line.split_once('#') {
                    ln
                } else {
                    line
                }
            })
            .peekable();

        while let Some(line) = lines.next() {
            // We assume that the first line is a target (otherwise the Makefile is invalid).
            let (target, dependencies) = line.split_once(':').ok_or(MakeError::LineIsNotATarget)?;

            // If we found a target, we manually advance the `lines` iterator
            // until a non-tab-indented line (i.e. a line without commands)
            // is reached.
            let mut commands = Vec::new();
            while let Some(line) = lines.peek() {
                if line.starts_with('\t') {
                    commands.push(line.trim().to_string());
                    let _ = lines.next();
                } else {
                    break;
                }
            }

            targets.push(Target {
                name: target.to_owned(),
                dependencies: dependencies
                    .split_whitespace()
                    .map(|dep| dep.trim().to_string())
                    .collect(),
                commands,
            })
        }

        Ok(Self { targets })
    }

    // Build the target with name `target` including dependencies.
    fn make(&self, target: &str) -> Result<(), Box<dyn std::error::Error>> {
        let target = self
            .targets
            .iter()
            .find(|t| t.name == target)
            .ok_or(MakeError::NoSuchTarget)?;

        // Find all the dependencies and see if they are targets or required files.
        let deps = target.dependencies.iter().map(|dep| {
            match self.targets.iter().find(|t| &t.name == dep) {
                Some(target) => Dependency::Target(target),
                None => Dependency::File(dep),
            }
        });

        // Then build the dependencies or check if the file exists.
        for dep in deps {
            match dep {
                Dependency::Target(t) => self.make(&t.name)?,
                Dependency::File(f) => {
                    if !std::path::Path::new(f).exists() {
                        return Err(Box::new(MakeError::DependencyDoesNotExist));
                    }
                }
            }
        }
        target.make()?;

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Find and parse the Makefile.
    let makefile_src = std::fs::read_to_string("Makefile")?;
    let makefile = Makefile::from_str(&makefile_src)?;

    // If there are arguments given, build these targets in order.
    // Otherwise build the first target in the Makefile.
    let args = std::env::args();
    if args.len() > 1 {
        for arg in args.skip(1) {
            makefile.make(&arg)?;
        }
    } else {
        makefile.make(&makefile.targets.first().ok_or(MakeError::NoTargets)?.name)?;
    }
    Ok(())
}
