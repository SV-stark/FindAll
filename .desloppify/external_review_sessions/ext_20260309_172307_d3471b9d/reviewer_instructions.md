# External Blind Review Session

Session id: ext_20260309_172307_d3471b9d
Session token: 7e5975878d62a98289c463820eb0da92
Blind packet: D:\FindAll\.desloppify\review_packet_blind.json
Template output: D:\FindAll\.desloppify\external_review_sessions\ext_20260309_172307_d3471b9d\review_result.template.json
Claude launch prompt: D:\FindAll\.desloppify\external_review_sessions\ext_20260309_172307_d3471b9d\claude_launch_prompt.md
Expected reviewer output: D:\FindAll\.desloppify\external_review_sessions\ext_20260309_172307_d3471b9d\review_result.json

Happy path:
1. Open the Claude launch prompt file and paste it into a context-isolated subagent task.
2. Reviewer writes JSON output to the expected reviewer output path.
3. Submit with the printed --external-submit command.

Reviewer output requirements:
1. Return JSON with top-level keys: session, assessments, issues.
2. session.id must be `ext_20260309_172307_d3471b9d`.
3. session.token must be `7e5975878d62a98289c463820eb0da92`.
4. Include issues with required schema fields (dimension/identifier/summary/related_files/evidence/suggestion/confidence).
5. Use the blind packet only (no score targets or prior context).
