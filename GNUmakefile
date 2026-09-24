MAKEFLAGS += -rR
.SUFFIXES:

IMAGE_NAME := fae-kernel
CARGO ?= $(HOME)/.cargo/bin/cargo
QEMU ?= qemu-system-x86_64
# serial-* demos halt in the kernel; cap so `make` returns.
QEMU_TIMEOUT ?= 4
NASM ?= nasm
QEMUFLAGS ?= -m 256M -serial stdio -no-reboot -no-shutdown
OVMF_CODE ?= $(firstword $(wildcard \
	/usr/share/edk2/x64/OVMF_CODE.4m.fd \
	/usr/share/edk2/x64/OVMF_CODE.fd \
	/usr/share/edk2-ovmf/x64/OVMF_CODE.fd \
	/usr/share/OVMF/OVMF_CODE.fd))

.PHONY: all kernel iso qemu serial serial-fw serial-uefi qemu-uefi-limine fw efi clean distclean

all: iso

kernel:
	$(MAKE) -C kernel CARGO="$(CARGO)" limine-elf

fw/cerne-fw.bin: fw/cerne-fw.asm
	$(NASM) -f bin -o $@ $<
	@test $$(stat -c%s $@) -eq 65536

kernel/kernel.fw.bin:
	$(MAKE) -C kernel CARGO="$(CARGO)" fw-bin

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
	timeout --signal=KILL $(QEMU_TIMEOUT) \
		$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d -display none $(QEMUFLAGS) || true

# Our firmware + our kernel. No Limine. No OVMF.
serial-fw: fw/cerne-fw.bin kernel/kernel.fw.bin
	timeout --signal=KILL $(QEMU_TIMEOUT) \
		$(QEMU) -M pc -bios fw/cerne-fw.bin \
		-device loader,file=kernel/kernel.fw.bin,addr=0x200000 \
		-display none $(QEMUFLAGS) || true

OVMF_VARS_SRC ?= $(firstword $(wildcard \
	/usr/share/edk2/x64/OVMF_VARS.4m.fd \
	/usr/share/edk2/x64/OVMF_VARS.fd))

serial-uefi: efi
	rm -rf esp
	mkdir -p esp/EFI/BOOT
	cp -v kernel/BOOTX64.EFI esp/EFI/BOOT/
	cp -f $(OVMF_VARS_SRC) ovmf_vars.fd
	timeout --signal=KILL $(QEMU_TIMEOUT) \
		$(QEMU) -M q35 -display none $(QEMUFLAGS) \
		-drive if=pflash,format=raw,unit=0,readonly=on,file=$(OVMF_CODE) \
		-drive if=pflash,format=raw,unit=1,file=ovmf_vars.fd \
		-drive format=raw,file=fat:rw:esp || true

qemu-uefi-limine: $(IMAGE_NAME).iso
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -display none $(QEMUFLAGS) \
		-drive if=pflash,format=raw,unit=0,readonly=on,file=$(OVMF_CODE)

clean:
	$(MAKE) -C kernel clean
	rm -rf iso_root esp $(IMAGE_NAME).iso fw/cerne-fw.bin

distclean: clean
	rm -rf limine
