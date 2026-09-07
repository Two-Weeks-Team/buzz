# Desktop execution observation candidate

The authorized workflow-history response carries a token-free observation.
Desktop validates and whitelists it rather than treating `running` as liveness.
Missing or invalid observations are unavailable. Legacy unowned executions are
unverified; expired or abandoned owned runs are outcome unknown. Neither state
offers automatic replay. A lease proves validity at the relay observation only,
not worker health, progress, or external effect completion.

The expanded history card displays generation, observation time and any lease
deadline. Its one-second local clock marks observations older than 30 seconds,
more than five seconds in the future, or past their deadline as requiring refresh.
The row badge is a render-time summary; the expanded notice ages independently.
Clock disagreement is conservative, not proof of server failure. Refresh invokes
the existing history query only, never trigger or approval submission.

Parser/presentation and SSR tests cover invalid and absent data, token stripping,
staleness, clock disagreement and unknown/unverified copy. Isolated Chromium
rehearsals use the real E2E bundle with synthetic IPC responses; they do not prove
native GUI dispatch or production behavior. Actual signed relay process-crash
evidence is recorded separately in the private operations repository.
