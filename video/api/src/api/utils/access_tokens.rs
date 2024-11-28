use std::collections::HashMap;

use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{AccessTokenScope, Resource};
use tonic::Status;
use video_common::database::AccessToken;

pub struct RequiredScope(pub Vec<AccessTokenScope>);

pub struct ResourcePermission(Option<Resource>, Permission);

impl From<(Option<Resource>, Permission)> for ResourcePermission {
	fn from((resource, permission): (Option<Resource>, Permission)) -> Self {
		Self(resource, permission)
	}
}

impl From<(Resource, Permission)> for ResourcePermission {
	fn from((resource, permission): (Resource, Permission)) -> Self {
		Self(Some(resource), permission)
	}
}

impl From<ResourcePermission> for RequiredScope {
	fn from(rp: ResourcePermission) -> Self {
		Self(vec![rp.into()])
	}
}

impl From<ResourcePermission> for AccessTokenScope {
	fn from(rp: ResourcePermission) -> Self {
		Self {
			resource: rp.0.map(|r| r.into()),
			permission: vec![rp.1.into()],
		}
	}
}

impl From<Vec<ResourcePermission>> for RequiredScope {
	fn from(rp: Vec<ResourcePermission>) -> Self {
		Self(rp.into_iter().map(|rp| rp.into()).collect())
	}
}

impl std::str::FromStr for ResourcePermission {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut split = s.splitn(2, ':');

		let resource = split.next().ok_or(())?.to_lowercase();
		let permission = split.next().ok_or(())?.to_lowercase();

		let resource = match resource.as_str() {
			"all" => None,
			"access_token" => Some(Resource::AccessToken),
			"events" => Some(Resource::Event),
			"playback_key_pair" => Some(Resource::PlaybackKeyPair),
			"playback_session" => Some(Resource::PlaybackSession),
			"recording" => Some(Resource::Recording),
			"room" => Some(Resource::Room),
			"s3_bucket" => Some(Resource::S3Bucket),
			"transcoding_config" => Some(Resource::TranscodingConfig),
			_ => return Err(()),
		};

		let permission = match permission.as_str() {
			"read" => Permission::Read,
			"write" => Permission::Write,
			"modify" => Permission::Modify,
			"delete" => Permission::Delete,
			"create" => Permission::Create,
			"events" => Permission::Events,
			"admin" => Permission::Admin,
			_ => return Err(()),
		};

		Ok(Self(resource, permission))
	}
}

impl RequiredScope {
	pub fn optimize(self) -> Self {
		let mut scopes = self.0;

		scopes.dedup();

		let mut scopes = scopes.into_iter().fold(HashMap::new(), |mut map, new_scope| {
			let resource = new_scope.resource;

			let scope = map.entry(resource).or_insert_with(|| AccessTokenScope {
				resource,
				permission: Vec::new(),
			});

			if scope.permission.contains(&Permission::Admin.into()) {
				return map;
			}

			if new_scope.permission.contains(&Permission::Admin.into()) {
				scope.permission = vec![Permission::Admin.into()];
				return map;
			}

			scope.permission.extend(new_scope.permission);

			scope.permission.sort_unstable();
			scope.permission.dedup();

			map
		});

		if let Some(global_scope) = scopes.remove(&None) {
			if global_scope.permission.contains(&Permission::Admin.into()) {
				return Self(vec![AccessTokenScope {
					resource: None,
					permission: vec![Permission::Admin.into()],
				}]);
			}

			for scope in scopes.values_mut() {
				scope.permission.retain(|p| !global_scope.permission.contains(p));
			}

			scopes.insert(None, global_scope);
		}

		let mut scopes = scopes.into_values().filter(|s| !s.permission.is_empty()).collect::<Vec<_>>();

		scopes.sort_unstable_by(|a, b| {
			a.resource
				.as_ref()
				.and_then(|a| Resource::try_from(*a).ok())
				.cmp(&b.resource.as_ref().and_then(|b| Resource::try_from(*b).ok()))
		});

		Self(scopes)
	}

	pub fn string_vec(&self) -> Vec<String> {
		let mut permissions = Vec::new();

		for ps in &self.0 {
			let scope = ps
				.resource
				.and_then(|s| Resource::try_from(s).ok())
				.map_or_else(|| "all".to_string(), |r| r.as_str_name().to_lowercase());

			permissions.extend(
				ps.permission
					.iter()
					.filter_map(|p| Permission::try_from(*p).ok())
					.map(|p| format!("{}:{}", scope, p.as_str_name().to_lowercase())),
			);
		}

		permissions.sort();

		permissions
	}
}

impl From<Vec<AccessTokenScope>> for RequiredScope {
	fn from(scopes: Vec<AccessTokenScope>) -> Self {
		Self(scopes)
	}
}

impl std::fmt::Display for RequiredScope {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.string_vec().join(" + ").fmt(f)
	}
}

pub trait AccessTokenExt {
	fn has_scope(&self, required: &RequiredScope) -> tonic::Result<()>;
}

impl AccessTokenExt for AccessToken {
	fn has_scope(&self, required: &RequiredScope) -> tonic::Result<()> {
		if required.0.iter().all(|required| {
			self.scopes.iter().any(|scope| {
				// Check that the scope is for all resources (unset) or matches the resource in
				// the required scope
				(scope.resource.is_none() || scope.resource == required.resource) &&
                // Check that the scope either has the Admin permission or has all of the required permissions
                (scope.permission.contains(&Permission::Admin.into()) || required.permission.iter().all(|p| scope.permission.contains(p)))
			})
		}) {
			Ok(())
		} else {
			Err(Status::permission_denied(format!("missing required scope: {required}")))
		}
	}
}
