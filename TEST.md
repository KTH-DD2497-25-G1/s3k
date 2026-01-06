# Testing instructions for the encrypted s3k filesystem

## Prerequisites
- Ensure you have Rustup installed.
- Install the toolchains:
```sh
cd projects/fs
rustup install nightly
rustup override set nightly
rustup target add riscv64gc-unknown-none-elf
```
- Generate disk image:
```sh
make disk-image
```

## Running the fs with a demo user app
```sh
make qemu
```

## First run: enabling encryption and write to disk
```sh
enc_on <your_pin>
open <filename> rw
write <fd> <your content>
close <fd>
```

## Second run: read from disk
```sh
unlock <your_pin>
open <filename> r
read <fd> <buffer_len>
close <fd>
```