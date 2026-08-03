#!/usr/bin/env julia
# Watches the events file and re-prints a report whenever its size changes.
# Path comes from EVENTS_FILE (default /data/events.jsonl) or ARGS[1].
include("src/ESP32Analysis.jl")
using .ESP32Analysis

# Wrapped in a function so `last_size` lives in proper (function) scope — a bare
# top-level `while` is a soft scope, where reassigning a global inside the loop is
# ambiguous and throws UndefVarError.
function watch(path::AbstractString, interval::Float64)
    println("watching ", path, " every ", interval, "s")
    flush(stdout)
    last_size = -1
    while true
        if isfile(path)
            sz = filesize(path)
            if sz != last_size
                last_size = sz
                try
                    println("\n" * "="^60)
                    print(report(path))
                    flush(stdout)
                catch err
                    @warn "report failed" error = err
                end
            end
        end
        sleep(interval)
    end
end

path = length(ARGS) >= 1 ? ARGS[1] : get(ENV, "EVENTS_FILE", "/data/events.jsonl")
interval = parse(Float64, get(ENV, "REPORT_INTERVAL", "5"))
watch(path, interval)
