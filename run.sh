#!/bin/sh

cargo build --lib --release --no-default-features && gcc -g -lrt -lpthread -ldl -ludev -Wall -O2 test.c target/release/libsmov.a && ./a.out
