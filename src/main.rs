//! A subset of the `make` utility.

use std::{
    cell::{RefCell, UnsafeCell},
    collections::HashMap,
    rc::Rc,
};

trait Get: Sized {
    type Output;
    fn get(&self) -> &Self::Output;
}

impl<'a> Get for Rc<UnsafeCell<Target<'a>>> {
    type Output = Target<'a>;

    fn get(&self) -> &Self::Output {
        unsafe { &*UnsafeCell::get(self) }
    }
}

/// A [Makefile] is represented as a list of [Target]s.
#[derive(Debug)]
struct Makefile<'a> {
    targets: Vec<Rc<UnsafeCell<Target<'a>>>>,
}

/// A Target's dependency. Can be another [Target] or a file.
#[derive(Debug)]
enum Dependency<'a> {
    Target(Rc<UnsafeCell<Target<'a>>>),
    File(String),
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
struct Target<'a> {
    name: String,
    dependencies: RefCell<Vec<Dependency<'a>>>,
    commands: Vec<String>,
}

impl<'a> std::fmt::Debug for Target<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Target")
            .field("name", &self.name)
            .field("dependencies", &self.dependencies.borrow())
            .field("commands", &self.commands)
            .finish()
    }
}

impl<'a> Target<'a> {
    /// Build this target. Assumes that dependencies
    /// have already been built and are valid.
    fn make(&self) -> Result<(), Box<dyn std::error::Error>> {
        for dep in self.dependencies.borrow().iter() {
            match dep {
                Dependency::Target(t) => t.get().make()?,
                Dependency::File(f) => {
                    if !std::path::Path::new(f).exists() {
                        return Err(Box::new(MakeError::DependencyDoesNotExist));
                    }
                }
            }
        }

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

impl<'a> Makefile<'a> {
    /// Parse a Makefile from a string.
    fn from_str(data: &'a str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut targets = Vec::new();
        let mut deps = HashMap::new();

        // First, we split the input into lines
        // and filter out the empty ones and comments.
        // We also filter out inline comments.
        let mut lines = data
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

            deps.insert(
                target,
                dependencies
                    .split_whitespace()
                    .map(|dep| dep.trim().to_string())
                    .collect::<Vec<_>>(),
            );

            targets.push(Rc::new(UnsafeCell::new(Target {
                name: target.to_owned(),
                dependencies: RefCell::new(Vec::new()),
                commands,
            })));
        }

        for target in &targets {
            let dependencies = deps
                .remove(target.get().name.as_str())
                .unwrap()
                .into_iter()
                .map(
                    |target_name| match targets.iter().find(|t| t.get().name == target_name) {
                        Some(t) => Dependency::Target(t.clone()),
                        None => Dependency::File(target_name),
                    },
                );
            target
                .get()
                .dependencies
                .borrow_mut()
                .append(&mut dependencies.collect::<Vec<_>>());
        }

        Ok(Self { targets })
    }

    // Build the target with name `target` including dependencies.
    fn make(&self, target: &str) -> Result<(), Box<dyn std::error::Error>> {
        let target = self
            .targets
            .iter()
            .find(|t| t.get().name == target)
            .ok_or(MakeError::NoSuchTarget)?;

        target.get().make()?;

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
        let target = makefile.targets.first().ok_or(MakeError::NoTargets)?.get();
        makefile.make(target.name.as_str())?;
    }
    Ok(())
}
