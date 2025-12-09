.POSIX:
.SECONDARY:

PLATFORM?=${patsubst plat/%.mk, %, ${wildcard plat/*.mk}}

all: ${addsuffix .all, ${PLATFORM}}
clean: ${addsuffix .clean, ${PLATFORM}}

%.all: 
	${MAKE} -f build.mk PLATFORM=${@:.all=} all
	cargo +nightly build --manifest-path=rust/Cargo.toml --target riscv64gc-unknown-none-elf --features ${@:.all=}

%.clean:
	${MAKE} -f build.mk PLATFORM=${@:.clean=} clean
	cargo +nightly clean --manifest-path=rust/Cargo.toml

.PHONY: all clean
