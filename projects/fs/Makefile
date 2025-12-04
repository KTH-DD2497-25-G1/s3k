.POSIX:

export PLATFORM   ?=qemu_virt4
export ROOT       :=${abspath ../..}
export BUILD      :=${abspath build/${PLATFORM}}
export RUST_BUILD :=${abspath target/riscv64gc-unknown-none-elf/debug}
export S3K_CONF_H :=${abspath s3k_conf.h}

include ${ROOT}/tools.mk

APPS=fsd app1 app2

ELFS:=${BUILD}/kernel.elf ${patsubst %, ${RUST_BUILD}/%, ${APPS}}
INIT_ELFS:=${BUILD}/kernel.elf ${patsubst %, ${RUST_BUILD}/%, fsd app1}

all: kernel apps

clean:
	@cargo clean

common:
	@${MAKE} -C ${ROOT}/common

kernel: common
	@${MAKE} -C ${ROOT}/kernel

apps:
	@cargo build

qemu: kernel apps
	@ELFS="${INIT_ELFS}" ${ROOT}/scripts/qemu.sh -drive file=fs.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

qemu-gdb: kernel apps
	@ELFS="${ELFS}" ${ROOT}/scripts/qemu.sh -gdb tcp::3333 -S -drive file=fs.img,if=none,format=raw,id=x0 -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0

gdb: kernel apps
	@ELFS="${ELFS}" ${ROOT}/scripts/gdb.sh

gdb-openocd: kernel apps
	@ELFS="${INIT_ELFS}" ${ROOT}/scripts/gdb-openocd.sh

disk-image: apps
	qemu-img create fs.img 100M
	mformat -i fs.img -F ::
	mcopy -i fs.img ${RUST_BUILD}/app2 ::/app2

disk-read:
	mdir -i fs.img
	mcopy -i fs.img ::/NEWFILE.TXT extracted.txt
	cat < extracted.txt
	rm extracted.txt

size: ${ELFS}
	${SIZE} ${ELFS}

.PHONY: all clean size qemu qemu-gdb gdb kernel common apps
