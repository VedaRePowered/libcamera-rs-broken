CARGO_ENCODED_RUSTFLAGS=`echo -e "-v\x1f-Clink-args=--target=armv7-unknown-linux-gnueabihf -static-libgcc -static-libstdc++"` AR=llvm-ar-12 TARGET_AR=$AR HOST=armv7-unknown-linux-gnueabihf CROSS_COMPILE=armv7-unknown-linux-gnueabihf cargo build -vvvv --target armv7-unknown-linux-gnueabihf --tests
