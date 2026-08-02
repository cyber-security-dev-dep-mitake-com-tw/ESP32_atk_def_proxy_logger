# Julia analysis image. Precompiles dependencies at build time, then runs against
# the shared /data volume written by the backend. By default it watches the events
# file and re-prints a report whenever it grows; override the command to run once.
FROM julia:1.10-bookworm
WORKDIR /app

# Project.toml declares ESP32Analysis as a package, so its source must be present
# before precompiling. Copy the manifest + source together, then instantiate.
COPY Project.toml ./
COPY src ./src
RUN julia --project=. -e 'using Pkg; Pkg.instantiate(); Pkg.precompile()'

COPY test ./test
COPY run_report.jl watch_report.jl ./

# Shared capture data mounted from the backend.
VOLUME ["/data"]
ENV EVENTS_FILE=/data/events.jsonl

# Watch mode: re-report as new events arrive. Run once instead with:
#   docker compose run --rm analysis julia --project=. run_report.jl /data/events.jsonl
CMD ["julia", "--project=.", "watch_report.jl"]
