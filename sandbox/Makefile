all: foo bar hello.txt

# I am a comment

foo: foo.c # What a nice target
	cc -o foo foo.c

bar: bar.c
	cc -o bar bar.c

hello.txt:
	echo "Hello, make!" > hello.txt

clean:
	rm -f foo
	rm -f bar
	rm -f hello.txt