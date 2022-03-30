clang -c driver.c -o driver.o -target x86_64-unknown-linux
ld.lld driver.o -entry main -o driver -r
