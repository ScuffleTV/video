import { graphql } from "$/gql";
import type { Client } from "@urql/svelte";

export function getUser(client: Client) {
	return client
		.query(
			graphql(`
				query GetUser {
					user {
						resp: withCurrentContext {
							id
							displayName
							displayColor {
								color
								hue
								isGray
							}
							username
							profilePicture {
								id
								variants {
									width
									height
									scale
									url
									format
									byteSize
								}
							}
							email
							emailVerified
							lastLoginAt
							channel {
								id
								live {
									roomId
								}
							}
							totpEnabled
						}
					}
				}
			`),
			{},
			{ requestPolicy: "network-only" },
		)
		.toPromise();
}

export async function logout(client: Client, token?: string | null) {
	await client
		.mutation(
			graphql(`
				mutation Logout($token: String) {
					auth {
						logout(sessionToken: $token)
					}
				}
			`),
			{ token },
			{ requestPolicy: "network-only" },
		)
		.toPromise();
}
