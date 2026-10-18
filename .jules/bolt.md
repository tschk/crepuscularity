# Performance notes

- ZPUI `host.collection` is called during both host validation and element construction. Its duplicate-key check previously compared every earlier row, making keyed lists quadratic before widget layout. Keep per-call key validation linear, preserve original row order and return borrowed storage, and retain allocation-free validation as the memory-pressure fallback. The standalone `plugins/zpui/tests/key_scaling.zig` measures this seam without requiring a graphics backend; it does not measure complete frames.
