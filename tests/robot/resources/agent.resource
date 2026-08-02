*** Settings ***
Documentation     Shared keywords for driving the Go control agent in demo mode.
Library           Process
Library           RequestsLibrary
Library           OperatingSystem

*** Variables ***
# Path to the prebuilt agent binary, resolved from the directory robot is invoked
# in (the repo root in CI and the documented local workflow). Override with
# `robot -v AGENT_BIN:/abs/path ...` if you run from elsewhere.
${AGENT_BIN}      ${EXECDIR}/backend/bin/agent
${BASE_URL}       http://127.0.0.1:8091
${DATA_DIR}       ${TEMPDIR}/esp32-robot-data
# When True (local/CI), the suite spawns its own agent binary. Set False to test an
# already-running backend (e.g. the docker-compose container):
#   robot -v START_AGENT:False -v BASE_URL:http://backend:8080 ...
${START_AGENT}    ${True}

*** Keywords ***
Start Agent In Demo Mode
    [Documentation]    Ensures a demo-mode agent is reachable at ${BASE_URL}. When
    ...                START_AGENT is True it launches the local binary; otherwise
    ...                it just waits for an externally-running backend to be healthy.
    IF    ${START_AGENT}
        Create Directory    ${DATA_DIR}
        ${proc}=    Start Process    ${AGENT_BIN}    --demo    --addr    :8091    --data    ${DATA_DIR}
        ...    stdout=${TEMPDIR}/agent.out    stderr=${TEMPDIR}/agent.err
        Set Suite Variable    ${AGENT_PROC}    ${proc}
    ELSE
        Set Suite Variable    ${AGENT_PROC}    ${None}
    END
    Wait Until Keyword Succeeds    30x    1s    Agent Is Healthy

Agent Is Healthy
    ${resp}=    GET    ${BASE_URL}/api/health
    Should Be Equal As Integers    ${resp.status_code}    200
    Should Be Equal    ${resp.json()}[status]    ok

Stop Agent
    IF    ${START_AGENT}
        Terminate Process    ${AGENT_PROC}
    END

Set Lab Mode
    [Arguments]    ${on}
    ${body}=    Create Dictionary    on=${on}
    ${resp}=    POST    ${BASE_URL}/api/safety/labmode    json=${body}
    RETURN    ${resp}

Send Command
    [Arguments]    ${node}    &{cmd}
    ${resp}=    POST    ${BASE_URL}/api/nodes/${node}/command    json=${cmd}
    ...    expected_status=any
    RETURN    ${resp}
