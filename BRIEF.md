# Brief

Stage 1. `C1 Frame the problem and set the rules`.

## The request, word for word (Sam, 2026-10-09)

> look at builder and the sgilson7.github.io website contents first; you can find all of the repos locally here on the host computer by going up to the directories tab. I want to make a new game using the learnings from the 3D graphics engine and implementation for Iron Vector. I received this from a peer in my lab today, and I want to surprise them with a made game: "Check this out: https://carolinatheatre.org/events/live-event/cirque-kikasse/ - vote for @Sam Gilson to turn it into a computer game". Please do research on the cirque kikasse, then build a 3D game where you are the performer having to do the various tricks and stuff the performers do. You are in 3rd person, and each mission is a different trick or combination of tricks. the body engine and physics should be derived from vagrancy, alongside potential costumes and mission selection and hasse diagram mission layout and scene art for each mission that matches the location of the performance across the US, starting at the one my peer called out and going to all of their different venues. there should be a crowd that yells stuff like the cup fill grade callouts from slushlune (excellent, oh no, darn). i essentially want 3Dvagrancy thats all about doing cool tricks on trampolines and other performer stuff like trapeezes, etc. please work until an mvp is finished and deployed to my website. i approve the creation of public github repos and pages to host the game. i approve deployments of the website ahead of time. any time you dont know the answer and need me, try and use the claude skills i have to inform your answer and choose your recommendation if that is not clear / yours is better. please continue working until the mvp is deployed and ready. i want to surprise my peer with this and essentially convince my lab the power of this builder platform i want to publish about by making his game in a single day and sending my peer a link to play it

## Autonomy

The plan is approved in advance: the request says to keep working until the MVP is deployed. The agent may create the public repository `sgilson7/food-truck-circus`, turn on GitHub Pages for it, push to `main` (which deploys through the checked workflow), and add the game to the scholar site and deploy that site. Where the request is silent, the agent decides using Sam's skills (`shared-technical-setting`, `agent-run-build`, `prove-the-tests`, `sam-gilson-voice`) and records the decision as a notebook row in `SECOND-ORDER.md`. The agent may not publish anything that presents itself as Cirque Kikasse's own work: no logo, no performer likenesses, no copied promotional text.

A later grant (Sam, 2026-10-19), word for word: "ok execute all of the changes you recommend to increase feasabilitiy, i approve make deploy in all builder made repos, and make deploy in the website, keep track of all changes made in the design log here". Scope: this run, for the changes in TRIAGE T1 to the Circus Math Lab; it covers pushing them to `main` (which deploys to Pages through the checked workflow) and the website deploy.

## Done means

- A third-person 3D game in the browser: the player is a food-truck circus performer.
- Missions are tricks or combinations of tricks, laid out as a Hasse diagram, set at the venues of the SANTÉ! tour, starting with the Carolina Theatre of Durham run.
- The body and physics derive from Vagrancy: integer Verlet particles and sticks, relaxed in a fixed order.
- Costumes, mission selection, a crowd that calls out grades (EXCELLENT ... OH NO, DARN).
- `./scripts/check.sh` passes; the game is live on GitHub Pages and linked from the scholar site.
