*** Settings ***
Documentation     End-to-end tests of the control agent REST API in demo mode.
Resource          ../resources/agent.resource
Suite Setup       Start Agent In Demo Mode
Suite Teardown    Stop Agent

*** Test Cases ***
Health Endpoint Reports OK
    Agent Is Healthy

All Three Demo Nodes Are Present
    ${resp}=    GET    ${BASE_URL}/api/nodes
    Should Be Equal As Integers    ${resp.status_code}    200
    ${ids}=    Evaluate    sorted([n['id'] for n in $resp.json()])
    Should Be Equal    ${ids}    ${{['node1','node2','node3']}}

Node1 Records Packets Over Time
    # The demo driver injects packets continuously; the packet counter must grow.
    ${first}=    Get Node Packet Count    node1
    Sleep    1s
    ${second}=    Get Node Packet Count    node1
    Should Be True    ${second} > ${first}    Node1 packet count did not increase

Safety Defaults To Lab Mode Off
    ${resp}=    GET    ${BASE_URL}/api/safety
    Should Be Equal    ${resp.json()}[lab_mode]    ${False}

*** Keywords ***
Get Node Packet Count
    [Arguments]    ${node}
    ${resp}=    GET    ${BASE_URL}/api/nodes
    ${count}=    Evaluate    next(n['packets'] for n in $resp.json() if n['id']=='${node}')
    RETURN    ${count}
