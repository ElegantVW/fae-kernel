MAKEFLAGS += -rR
.SUFFIXES:

CARGO ?= $(HOME)/.cargo/bin/cargo
RUST_TARGET := x86_64-unknown-none
RUST_PROFILE ?= dev
ifeq ($(RUST_PROFILE),dev)
    RUST_PROFILE_SUBDIR := debug
else
    RUST_PROFILE_SUBDIR := $(RUST_PROFILE)
endif

.PHONY: all clean

all:
	RUSTFLAGS="-C relocation-model=static" $(CARGO) build --target $(RUST_TARGET) --profile $(RUST_PROFILE)
	cp -v target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR)/fae-kernel kernel

clean:
	$(CARGO) clean
	rm -f kernel
