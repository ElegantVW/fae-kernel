MAKEFLAGS += -rR
.SUFFIXES:

IMAGE_NAME := fae-kernel
CARGO ?= $(HOME)/.cargo/bin/cargo
QEMU ?= qemu-system-x86_64
# serial-* demos halt in the kernel; cap so `make` returns.
QEMU_TIMEOUT ?= 4
NASM ?= nasm
QEMUFLAGS ?= -m 256M -serial stdio -no-reboot -no-shutdown
LD_BIN := ld/cerne-ld.bin
IMAGE := kindling.img
OVMF_CODE ?= $(firstword $(wildcard \
	/usr/share/edk2/x64/OVMF_CODE.4m.fd \
	/usr/share/edk2/x64/OVMF_CODE.fd \
	/usr/share/edk2-ovmf/x64/OVMF_CODE.fd \
	/usr/share/OVMF/OVMF_CODE.fd))

.PHONY: all kernel iso qemu serial serial-fw serial-uefi qemu-uefi-limine fw efi clean distclean
.PHONY: help flint steel tinder hearth kindle hearth-see audit below below-ten image test

help:
	@echo "Kindling — how one lights it"
	@echo "  make flint     nasm the firmware (the striker)"
	@echo "  make steel     cargo-cast the kernel crystal"
	@echo "  make tinder    firmware ROM that catches"
	@echo "  make image     cast the boot disk (KMAP + loader + kernel)"
	@echo "  make hearth    QEMU bowl (alias: serial-fw)"
	@echo "  make kindle    flint + steel + hearth — paved fire"
	@echo "  make hearth-see   same fire, window (lilac VGA)"
	@echo "  make audit     several bowls; fail if a line is missing"
	@echo "  make below     full below gate (audit+trap+fmap+fw-trap+efi)"
	@echo "  make below-ten    gate × 10"
	@echo "  make test      throwaway VMs (happy..spawn) + grove on VGA"
	@echo "  make serial-uefi   other people's firmware, our clothes"
	@echo "  make serial        borrowed match (Limine)"
	@echo "  make distclean     the forest forgets; the seed does not"
	@echo "mana: B thimble · KiB cup · MiB bowl · GiB well"
	@echo "steel uses $(CARGO) (nightly). /usr/bin/cargo is the host stable."
	@echo "a spark asks; the forest answers in tools"

flint: fw/cerne-fw.bin
steel:
	$(MAKE) -C kernel CARGO="$(CARGO)" fw-bin
tinder: flint
hearth: serial-fw
kindle: serial-fw
image: $(IMAGE)
$(IMAGE): flint steel $(LD_BIN) scripts/mkimg.py
	python3 scripts/mkimg.py --loader $(LD_BIN) --kernel kernel/kernel.fw.bin --out $@

$(LD_BIN): ld/cerne-ld.asm
	$(NASM) -f bin -o $@ $<
audit:
	sh scripts/audit-kindle.sh
below:
	sh scripts/below.sh
below-ten:
	@i=1; while [ $$i -le 10 ]; do echo "==== below $$i/10 ===="; sh scripts/below.sh || exit 1; i=$$((i+1)); done; echo "kindling: below ten ok"
test:
	sh scripts/test-vm.sh happy
	sh scripts/test-vm.sh house
	sh scripts/test-vm.sh ring3
	sh scripts/test-vm.sh reclaim
	sh scripts/test-vm.sh tale
	sh scripts/test-vm.sh spawn
	python3 scripts/check-grove.py
	@echo "kindling: test ok"
hearth-see: image
	$(QEMU) -M pc -bios fw/cerne-fw.bin \
		-drive if=ide,format=raw,file=$(IMAGE) \
		$(QEMUFLAGS)

all: iso

kernel:
	$(MAKE) -C kernel CARGO="$(CARGO)" limine-elf

fw/font8x16.bin: scripts/mkfont.py
	python3 scripts/mkfont.py

fw/cerne-fw.bin: fw/cerne-fw.asm fw/font8x16.bin
	$(NASM) -f bin -o $@ $<
	@test $$(stat -c%s $@) -eq 65536
	python3 scripts/romsum.py $@

# kernel.fw.bin is produced by `steel` (always recast).

efi:
	$(MAKE) -C kernel CARGO="$(CARGO)" efi

limine/limine:
	git clone https://codeberg.org/Limine/Limine.git --branch=v10.x-binary --depth=1 limine
	$(MAKE) -C limine

iso: $(IMAGE_NAME).iso

$(IMAGE_NAME).iso: limine/limine kernel
	rm -rf iso_root
	mkdir -p iso_root/boot/limine iso_root/EFI/BOOT
	cp -v kernel/kernel iso_root/boot/
	cp -v limine.conf iso_root/boot/limine/
	cp -v limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin iso_root/boot/limine/
	cp -v limine/BOOTX64.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs -R -r -J -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
	./limine/limine bios-install $(IMAGE_NAME).iso
	rm -rf iso_root

qemu: $(IMAGE_NAME).iso
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d $(QEMUFLAGS)

serial: $(IMAGE_NAME).iso
	timeout --foreground --signal=KILL $(QEMU_TIMEOUT) \
		$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d -display none $(QEMUFLAGS) || true

# Our firmware loads our loader off the disk; the loader loads our kernel.
# No -device loader, no Limine, no OVMF.
serial-fw: image
	timeout --foreground --signal=KILL $(QEMU_TIMEOUT) \
		$(QEMU) -M pc -bios fw/cerne-fw.bin \
		-drive if=ide,format=raw,file=$(IMAGE) \
		-display none $(QEMUFLAGS) || true

OVMF_VARS_SRC ?= $(firstword $(wildcard \
	/usr/share/edk2/x64/OVMF_VARS.4m.fd \
	/usr/share/edk2/x64/OVMF_VARS.fd))

serial-uefi: efi
	rm -rf esp
	mkdir -p esp/EFI/BOOT
	cp -v kernel/BOOTX64.EFI esp/EFI/BOOT/
	cp -f $(OVMF_VARS_SRC) ovmf_vars.fd
	timeout --foreground --signal=KILL $(QEMU_TIMEOUT) \
		$(QEMU) -M q35 -display none $(QEMUFLAGS) \
		-drive if=pflash,format=raw,unit=0,readonly=on,file=$(OVMF_CODE) \
		-drive if=pflash,format=raw,unit=1,file=ovmf_vars.fd \
		-drive format=raw,file=fat:rw:esp || true

qemu-uefi-limine: $(IMAGE_NAME).iso
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -display none $(QEMUFLAGS) \
		-drive if=pflash,format=raw,unit=0,readonly=on,file=$(OVMF_CODE)

clean:
	$(MAKE) -C kernel clean
	rm -rf iso_root esp $(IMAGE_NAME).iso fw/cerne-fw.bin kindling*.img $(LD_BIN)

distclean: clean
	rm -rf limine
	@echo "the forest forgets; the seed does not"

.DEFAULT:
	@echo "the path does not grow here · $@"
	@false
