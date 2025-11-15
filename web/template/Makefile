SASS ?= sass
PNPM ?= pnpm

CUSTOM_BOOTSTRAP_SCSS := vendor/custom-bootstrap/scss/custom.scss
CUSTOM_CSS_OUT := assets/css/styles.css
VENDOR_FOLDER := vendor/custom-bootstrap

.PHONY: css watch-css install

install:
	$(PNPM) -C $(VENDOR_FOLDER) install

css:
	$(SASS) -q $(CUSTOM_BOOTSTRAP_SCSS) $(CUSTOM_CSS_OUT)

watch-css:
	$(SASS) --watch $(CUSTOM_BOOTSTRAP_SCSS):$(CUSTOM_CSS_OUT)
