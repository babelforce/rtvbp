---
sidebar_position: 1
---

# Overview

`RTVBP` allows you to provide an endpoint which can extend the babelforce telephony
capabilities and easily integrate into your own applications.

---

## Terminology

### Client & Server

Throughout this document, **Client** refers to the **babelforce** cloud and its telephony infrastructure. The **Server** represents **your application**, which is configured to accept incoming connections from us. In the current version of the protocol, all sessions are initiated by the Client—that is, connections will always originate from the **babelforce** platform.

### Peer

When the specific direction of communication is not important, we may refer to either party—Client or Server—as a Peer. In this context, Peer is a generic term representing either side of the connection.

### Protocol

The Protocol defines a structured and well-documented set of message types exchanged between the Client and Server. These messages enable the two Peers to communicate reliably and consistently throughout the session.

### Transport Protocol

The Transport Protocol defines how data is physically transmitted between the Client and Server over the network. In this implementation, the transport layer is based on WebSockets over TLS, ensuring secure, bidirectional communication. While the Protocol itself defines the structure and meaning of the messages exchanged, the Transport Protocol is responsible for reliably delivering those messages in real time between the two Peers.
