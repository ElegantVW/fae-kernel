MAKEFLAGS += -rR
.SUFFIXES:

CARGO ?= $(HOME)/.cargo/bin/cargo
OBJCOPY ?= llvm-objcopy
RUST_TARGET := x86_64-unknown-none
UEFI_TARGET := x86_64-unknown-uefi
RUST_PROFILE ?= dev
ifeq ($(RUST_PROFILE),dev)
    RUST_PROFILE_SUBDIR := debug
else
    RUST_PROFILE_SUBDIR := $(RUST_PROFILE)
endif

NONE_DIR := target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR)
UEFI_DIR := target/$(UEFI_TARGET)/$(RUST_PROFILE_SUBDIR)

.PHONY: all limine-elf fw-bin efi clean

all: limine-elf

limine-elf:
	RUSTFLAGS="-C relocation-model=static" $(CARGO) build --target $(RUST_TARGET) --bin fae-kernel --profile $(RUST_PROFILE)
	cp -v $(NONE_DIR)/fae-kernel kernel

fw-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.fw.bin

trap6-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,trap6 --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.trap.bin

efi:
	$(CARGO) build --target $(UEFI_TARGET) --bin cerne-efi --features efi --profile $(RUST_PROFILE)
	cp -v $(UEFI_DIR)/cerne-efi.efi BOOTX64.EFI

clean:
	$(CARGO) clean
	rm -f kernel kernel.fw.bin kernel.trap.bin BOOTX64.EFI
