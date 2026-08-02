"""
    ESP32Analysis

Offline analysis of the events.jsonl written by the Go agent: channel utilization,
top talkers, RSSI distribution, and deauth-attack timelines. Pure functions so the
logic is unit-testable; `report` ties them together for CLI use.
"""
module ESP32Analysis

using JSON
using Statistics

export load_events, channel_histogram, deauth_summary, rssi_stats, top_bssids, report

"""
    load_events(path) -> Vector{Dict}

Read a JSONL events file into a vector of records. Blank lines are skipped.
Each record is `{"node_id": ..., "event": {...}}`.
"""
function load_events(path::AbstractString)
    records = Dict[]
    for line in eachline(path)
        isempty(strip(line)) && continue
        push!(records, JSON.parse(line))
    end
    return records
end

"Return only the events of a given `ev` type (e.g. \"packet\", \"deauth_alert\")."
function events_of(records, ev::AbstractString)
    [r["event"] for r in records if get(r["event"], "ev", "") == ev]
end

"""
    channel_histogram(records) -> Dict{Int,Int}

Count captured packets per channel across all `packet` events.
"""
function channel_histogram(records)
    hist = Dict{Int,Int}()
    for e in events_of(records, "packet")
        ch = get(e, "ch", 0)
        hist[ch] = get(hist, ch, 0) + 1
    end
    return hist
end

"""
    rssi_stats(records) -> NamedTuple

Mean/min/max RSSI over packet events, or all-zero when there are none.
"""
function rssi_stats(records)
    vals = Float64[]
    for e in events_of(records, "packet")
        haskey(e, "rssi") && push!(vals, float(e["rssi"]))
    end
    isempty(vals) && return (n=0, mean=0.0, min=0.0, max=0.0)
    return (n=length(vals), mean=mean(vals), min=minimum(vals), max=maximum(vals))
end

"""
    deauth_summary(records) -> Dict{String,Int}

Total deauth-alert count per targeted BSSID.
"""
function deauth_summary(records)
    summary = Dict{String,Int}()
    for e in events_of(records, "deauth_alert")
        bssid = get(e, "bssid", "?")
        summary[bssid] = get(summary, bssid, 0) + get(e, "count", 1)
    end
    return summary
end

"""
    top_bssids(records, n=5) -> Vector{Pair{String,Int}}

The `n` most-attacked BSSIDs, highest first.
"""
function top_bssids(records, n::Int=5)
    pairs = collect(deauth_summary(records))
    sort!(pairs, by=p -> p.second, rev=true)
    return pairs[1:min(n, length(pairs))]
end

"""
    report(path) -> String

Human-readable text report for the events file at `path`.
"""
function report(path::AbstractString)
    records = load_events(path)
    io = IOBuffer()
    println(io, "ESP32 capture report: ", path)
    println(io, "records: ", length(records))
    println(io)
    println(io, "Channel utilization (packets/channel):")
    for (ch, cnt) in sort(collect(channel_histogram(records)))
        println(io, "  ch ", ch, ": ", cnt)
    end
    r = rssi_stats(records)
    println(io)
    println(io, "RSSI: n=", r.n, " mean=", round(r.mean, digits=1),
        " min=", r.min, " max=", r.max)
    println(io)
    println(io, "Deauth alerts by BSSID:")
    for (bssid, cnt) in top_bssids(records, 10)
        println(io, "  ", bssid, ": ", cnt)
    end
    return String(take!(io))
end

end # module
