.POSIX:
.SECONDARY:

PLATFORM?=${patsubst plat/%.mk, %, ${wildcard plat/*.mk}}

all: ${addsuffix .all, ${PLATFORM}}
clean: ${addsuffix .clean, ${PLATFORM}}

%.all: 
	${MAKE} -f build.mk PLATFORM=${@:.all=} all
	cargo build --manifest-path=rust/Cargo.toml --target riscv64gc-unknown-none-elf --features ${@:.all=}

%.clean:
	${MAKE} -f build.mk PLATFORM=${@:.clean=} clean
	cargo clean --manifest-path=rust/Cargo.toml

.PHONY: all clean
