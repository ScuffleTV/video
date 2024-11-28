# Scuffle Design

Scuffle is a comprehensive system, segmented into several distinct components:

## [Video](./video/README.md)

**Scuffle Video** serves as a Live Streaming as a Service (LSaaS) product. It offers an API that facilitates the creation of rooms and provides various configuration options to manage transcoding and recording processes. Designed for self-hosting, Scuffle Video can be deployed on a Kubernetes cluster, a standalone server, or any desired infrastructure. Its flexibility is further highlighted by an embeddable video player that can be integrated into any website. Notably, Scuffle Video operates independently of the Scuffle Platform, meaning you can utilize the video component without the streaming platform.

## [Platform](./platform/README.md)

**Scuffle Platform** is a live streaming ecosystem where users can create accounts, engage in chat, watch live streams, upload profile pictures, emotes, and more. It leans more towards being a social media platform and serves as a frontend for the video platform. While it's tailored to meet Scuffle's specific requirements, like all other components, it can be self-hosted if desired.

## [CDN](./cdn/README.md)

### DISCLAIMER: This service is currently in the conceptual phase and has not been developed yet.

A Content Delivery Network (CDN) is pivotal for delivering static assets such as video segments, images, etc. **Scuffle CDN**, a future addition to our suite, aims to redefine the standards of handling loads, with a particular focus on websockets and caching. For a deeper understanding of this envisioned service, refer to the [README.md](./cdn/README.md) in the CDN directory or engage with our community on [discord](https://discord.gg/scuffle) for inquiries.
