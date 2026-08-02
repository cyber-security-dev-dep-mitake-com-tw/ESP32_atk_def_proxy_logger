#!/usr/bin/env julia
# Usage: julia --project run_report.jl ../data/events.jsonl
include("src/ESP32Analysis.jl")
using .ESP32Analysis
path = length(ARGS) >= 1 ? ARGS[1] : "../data/events.jsonl"
print(report(path))
