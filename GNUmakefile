MAKEFLAGS += -rR
.SUFFIXES:

IMAGE_NAME := fae-kernel
CARGO ?= $(HOME)/.cargo/bin/cargo
QEMU ?= qemu-system-x86_64
QEMUFLAGS ?= -m 256M -serial stdio -no-reboot -no-shutdown

.PHONY: all kernel iso qemu serial clean distclean

all: iso

kernel:
	$(MAKE) -C kernel CARGO="$(CARGO)"

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
	$(QEMU) -M q35 -cdrom $(IMAGE_NAME).iso -boot d -display none $(QEMUFLAGS)

clean:
	$(MAKE) -C kernel clean
	rm -rf iso_root $(IMAGE_NAME).iso

distclean: clean
	rm -rf limine
