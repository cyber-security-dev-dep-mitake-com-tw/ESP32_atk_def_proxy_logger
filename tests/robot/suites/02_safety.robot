*** Settings ***
Documentation     Verifies the Node3 attack safety gate through the HTTP API.
...               The demo agent is started with no BSSID allowlist, so every
...               attack must be refused even after lab mode is enabled.
Resource          ../resources/agent.resource
Suite Setup       Start Agent In Demo Mode
Suite Teardown    Stop Agent

*** Test Cases ***
Attack Refused When Lab Mode Off
    Set Lab Mode    ${False}
    ${resp}=    Send Command    node3    cmd=attack    type=deauth
    ...    bssid=aa:bb:cc:dd:ee:ff    confirm_own_net=${True}
    Should Be Equal As Integers    ${resp.status_code}    403
    Should Contain    ${resp.json()}[error]    lab mode is disabled

Attack Refused Without Confirm Flag
    Set Lab Mode    ${True}
    ${resp}=    Send Command    node3    cmd=attack    type=deauth
    ...    bssid=aa:bb:cc:dd:ee:ff    confirm_own_net=${False}
    Should Be Equal As Integers    ${resp.status_code}    403
    Should Contain    ${resp.json()}[error]    confirm_own_net

Attack Refused For BSSID Not On Allowlist
    Set Lab Mode    ${True}
    ${resp}=    Send Command    node3    cmd=attack    type=deauth
    ...    bssid=aa:bb:cc:dd:ee:ff    confirm_own_net=${True}
    Should Be Equal As Integers    ${resp.status_code}    403
    Should Contain    ${resp.json()}[error]    allowlist

Non-Attack Commands Are Allowed
    ${resp}=    Send Command    node1    cmd=set_channel    ch=${11}
    Should Be Equal As Integers    ${resp.status_code}    200

Lab Mode Toggles On And Off
    ${on}=    Set Lab Mode    ${True}
    Should Be Equal    ${on.json()}[lab_mode]    ${True}
    ${off}=    Set Lab Mode    ${False}
    Should Be Equal    ${off.json()}[lab_mode]    ${False}
