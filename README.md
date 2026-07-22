# CrazyTime Server
> disclaimer, this project is not affiliated with the official CrazyTime brand/trademark, KOSMOS, or anything/anyone related to it, as it is just a fan project.

A (blazingly fast) crazytime server written in rust

### TODO
- Remove CloseLobby feature
- On player leave/disconnect, add back card pile to card pool
- fix minimum match players as 3, and cancel ongoing match if too few players
- once bots are in place, make a feature to replace disconnected players with bots if they are less than 3
- better error handling
- more bug hunting and actual user testing

### Missing features
- Lobby game spectating
- Game pausing by host
- Extended card possibilities/truly random CardPool match setting
- Usernames
- Bots
- Public/private lobby toggle
- Multiple hosts/lobby permissions
- Lobby join requests (requires being accepted by host)
- Game statistics
- Custom rules (with an embedded wasm runtime for running rule effects)

