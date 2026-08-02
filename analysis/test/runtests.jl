using Test
include("../src/ESP32Analysis.jl")
using .ESP32Analysis

# Build a small synthetic events file mirroring the Go store format.
mktemp() do path, io
    println(io, """{"node_id":"node1","event":{"ev":"packet","ch":6,"rssi":-42}}""")
    println(io, """{"node_id":"node1","event":{"ev":"packet","ch":6,"rssi":-50}}""")
    println(io, """{"node_id":"node1","event":{"ev":"packet","ch":11,"rssi":-60}}""")
    println(io, """{"node_id":"node2","event":{"ev":"deauth_alert","bssid":"aa:bb","count":12}}""")
    println(io, """{"node_id":"node2","event":{"ev":"deauth_alert","bssid":"aa:bb","count":3}}""")
    println(io, "")  # blank line should be skipped
    flush(io)

    records = load_events(path)
    @test length(records) == 5

    hist = channel_histogram(records)
    @test hist[6] == 2
    @test hist[11] == 1

    r = rssi_stats(records)
    @test r.n == 3
    @test r.min == -60.0
    @test r.max == -42.0

    ds = deauth_summary(records)
    @test ds["aa:bb"] == 15

    top = top_bssids(records)
    @test first(top).first == "aa:bb"

    txt = report(path)
    @test occursin("Channel utilization", txt)
    @test occursin("aa:bb", txt)
end

println("all analysis tests passed")
