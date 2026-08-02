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

*** Keywords ***
Start Agent In Demo Mode
    [Documentation]    Launches the prebuilt agent binary with synthetic nodes and
    ...                waits until its health endpoint responds.
    Create Directory    ${DATA_DIR}
    ${proc}=    Start Process    ${AGENT_BIN}    --demo    --addr    :8091    --data    ${DATA_DIR}
    ...    stdout=${TEMPDIR}/agent.out    stderr=${TEMPDIR}/agent.err
    Set Suite Variable    ${AGENT_PROC}    ${proc}
    Wait Until Keyword Succeeds    10x    0.5s    Agent Is Healthy

Agent Is Healthy
    ${resp}=    GET    ${BASE_URL}/api/health
    Should Be Equal As Integers    ${resp.status_code}    200
    Should Be Equal    ${resp.json()}[status]    ok

Stop Agent
    Terminate Process    ${AGENT_PROC}

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
