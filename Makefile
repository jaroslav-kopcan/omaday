PREFIX ?= $(HOME)/.local
APP_ID  = dev.rajo.omaday

build:
	cargo build --release

install: build
	install -Dm755 target/release/omaday $(PREFIX)/bin/omaday
	install -Dm644 data/$(APP_ID).desktop $(PREFIX)/share/applications/$(APP_ID).desktop
	install -Dm644 data/omaday.svg $(PREFIX)/share/icons/hicolor/scalable/apps/$(APP_ID).svg
	-update-desktop-database $(PREFIX)/share/applications 2>/dev/null

uninstall:
	rm -f $(PREFIX)/bin/omaday
	rm -f $(PREFIX)/share/applications/$(APP_ID).desktop
	rm -f $(PREFIX)/share/icons/hicolor/scalable/apps/$(APP_ID).svg

.PHONY: build install uninstall
