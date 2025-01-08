.PHONY: deps factorial fibonacci test clean run

CAIRO_1_FOLDER=../cairo_programs/cairo-1-programs

$(CAIRO_1_FOLDER)/%.trace: $(CAIRO_1_FOLDER)/%.cairo
	cargo run --release -F mod_builtin $< --trace_file $@ --layout all_cairo

$(CAIRO_1_FOLDER)/%.memory: $(CAIRO_1_FOLDER)/%.cairo
	cargo run --release -F mod_builtin $< --memory_file $@ --layout all_cairo

CAIRO_1_PROGRAMS=$(wildcard ../cairo_programs/cairo-1-programs/*.cairo)
TRACES:=$(patsubst $(CAIRO_1_FOLDER)/%.cairo, $(CAIRO_1_FOLDER)/%.trace, $(CAIRO_1_PROGRAMS))
MEMORY:=$(patsubst $(CAIRO_1_FOLDER)/%.cairo, $(CAIRO_1_FOLDER)/%.memory, $(CAIRO_1_PROGRAMS))

deps:
	git clone --depth=1 -b v2.10.0-rc.0 https://github.com/starkware-libs/cairo.git \
	&& mv cairo/corelib/ . \
	&& rm -rf cairo/

run: $(TRACES) $(MEMORY)

test:
	cargo test

clean:
	rm -rf corelib
	rm -rf cairo
	rm -rf ../cairo_programs/cairo-1-programs/*.memory
	rm -rf ../cairo_programs/cairo-1-programs/*.trace
