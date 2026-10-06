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

.PHONY: all limine-elf fw-bin efi efi-leaf efi-tale clean

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

house-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,house-test --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.house.bin

ring3-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,house-test,ring3-test --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.ring3.bin

reclaim-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,house-test,reclaim-test --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.reclaim.bin

tale-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,house-test,tale-test --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.tale.bin

spawn-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,house-test,spawn-test --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.spawn.bin

ingle-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,house-test,ingle-test --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.ingle.bin

leaf-bin:
	RUSTFLAGS="-C relocation-model=static" \
		$(CARGO) build --target $(RUST_TARGET) --bin fae-kernel-fw --features fw-link,house-test,leaf-test --profile $(RUST_PROFILE)
	$(OBJCOPY) -O binary $(NONE_DIR)/fae-kernel-fw kernel.leaf.bin

efi:
	$(CARGO) build --target $(UEFI_TARGET) --bin cerne-efi --features efi --profile $(RUST_PROFILE)
	cp -v $(UEFI_DIR)/cerne-efi.efi BOOTX64.EFI

efi-leaf:
	$(CARGO) build --target $(UEFI_TARGET) --bin cerne-efi --features efi,leaf-test --profile $(RUST_PROFILE)
	cp -v $(UEFI_DIR)/cerne-efi.efi BOOTX64.LEAF.EFI

efi-tale:
	$(CARGO) build --target $(UEFI_TARGET) --bin cerne-efi --features efi,tale-test --profile $(RUST_PROFILE)
	cp -v $(UEFI_DIR)/cerne-efi.efi BOOTX64.TALE.EFI

clean:
	$(CARGO) clean
	rm -f kernel kernel.fw.bin kernel.trap.bin kernel.house.bin kernel.ring3.bin kernel.reclaim.bin kernel.tale.bin kernel.spawn.bin kernel.ingle.bin kernel.leaf.bin BOOTX64.EFI BOOTX64.LEAF.EFI BOOTX64.TALE.EFI
