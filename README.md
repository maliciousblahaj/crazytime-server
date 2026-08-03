# CrazyTime Server
> disclaimer, this project is not affiliated with the official CrazyTime brand/trademark, KOSMOS, or anything/anyone related to it, as it is just a fan project.

A (blazingly fast) crazytime server written in rust

### TODO
- fix so default rules cannot be removed server side
- match cancel by host
- better error handling
- more bug hunting and actual user testing

- Determine incorrect i won to be either error or instant round termination, and what if a player is supposed to do i won and does something wrong and then i won
  correctly, the run should still terminate with error even if they made a correct i won within the error reaction time
- (maybe migrate from chrono to jiff)

### Missing features
- Lobby game spectating
- Game pausing by host (actually not needed since we can just let a round end, and disable auto start round)
- Extended card possibilities/truly random CardPool match setting
- Usernames
- Bots (once in place, maybe make a feature to replace disconnected players with bots if they are less than 3)
- Public/private lobby toggle
- Multiple hosts/lobby permissions
- Lobby join requests (requires being accepted by host)
- Game statistics
- Custom rules (with an embedded wasm runtime for running rule effects)

