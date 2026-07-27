## Builder Track Weekly Report — Week 5

**Name:** Telesphore TUGANIMANA <br>
**Week Ending:** 27-07-2026

### Courses Completed

- **Deploy the app**
  - Dockerized the Rust API (`Dockerfile`).
  - Set up GitHub Actions to build and push the image to DigitalOcean registry.
  - Deployed the app container to a Droplet (`deploy.yml`).

- **Fiber node deployment**
  - Wrote Fiber testnet config (`deploy/fiber/config.yml`) — P2P + RPC, bootnodes.
  - Automated Fiber deploy via GitHub Actions (`deploy-fiber.yml`).
  - Ran Fiber as a Docker container on the Droplet (ports `8228` / `8227`).
  - Wired the app to Fiber RPC (`FIBER_RPC_URL`) so `/fiber/*` works after deploy.

### Key Learnings

- **Fiber**
  - How a Fiber node is configured and announced on testnet.
  - Connecting the API to a remote Fiber RPC endpoint.
  - Docker + GitHub Actions for VPS deploy (secrets, volumes, port publish).

### Practical Progress

- Fiber node running on Droplet via CI deploy.
- App redeploy picks up Fiber RPC and can talk to the node.
- Address / transfer API work from prior weeks still in place against this setup.
-  deploy Rust app to digital ocean

### Environment Setup

- DigitalOcean Droplet + Container Registry.
- GitHub Actions secrets for Droplet SSH, Fiber key, and announced P2P addr.
- Fiber image: `nervos/fiber` (testnet).
