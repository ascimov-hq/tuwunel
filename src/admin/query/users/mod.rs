mod auth_ldap;
mod count_one_time_keys;
mod count_users;
mod get_all_backups;
mod get_backup_algorithm;
mod get_backup_session;
mod get_device_keys;
mod get_device_metadata;
mod get_devices_version;
mod get_latest_backup;
mod get_latest_backup_version;
mod get_master_key;
mod get_room_backups;
mod get_shared_rooms;
mod get_to_device_events;
mod get_user_signing_key;
mod iter_users;
mod list_devices;
mod list_devices_metadata;
mod password_hash;
mod search_ldap;

use clap::Subcommand;
use ruma::{OwnedDeviceId, OwnedRoomId, OwnedUserId};
use tuwunel_core::Result;

use crate::admin_command_dispatch;

#[admin_command_dispatch]
#[derive(Debug, Subcommand)]
/// All the getters and iterators from src/service/users/
pub(crate) enum UsersCommand {
	CountUsers,

	IterUsers {
		/// Only list historical user ids
		#[arg(long)]
		historical: bool,
	},

	PasswordHash {
		user_id: OwnedUserId,
	},

	ListDevices {
		user_id: OwnedUserId,
	},

	ListDevicesMetadata {
		user_id: OwnedUserId,
	},

	GetDeviceMetadata {
		user_id: OwnedUserId,
		device_id: OwnedDeviceId,
	},

	GetDevicesVersion {
		user_id: OwnedUserId,
	},

	CountOneTimeKeys {
		user_id: OwnedUserId,
		device_id: OwnedDeviceId,
	},

	GetDeviceKeys {
		user_id: OwnedUserId,
		device_id: OwnedDeviceId,
	},

	GetUserSigningKey {
		user_id: OwnedUserId,
	},

	GetMasterKey {
		user_id: OwnedUserId,
	},

	GetToDeviceEvents {
		user_id: OwnedUserId,
		device_id: OwnedDeviceId,
	},

	GetLatestBackup {
		user_id: OwnedUserId,
	},

	GetLatestBackupVersion {
		user_id: OwnedUserId,
	},

	GetBackupAlgorithm {
		user_id: OwnedUserId,
		version: String,
	},

	GetAllBackups {
		user_id: OwnedUserId,
		version: String,
	},

	GetRoomBackups {
		user_id: OwnedUserId,
		version: String,
		room_id: OwnedRoomId,
	},

	GetBackupSession {
		user_id: OwnedUserId,
		version: String,
		room_id: OwnedRoomId,
		session_id: String,
	},

	GetSharedRooms {
		user_a: OwnedUserId,
		user_b: OwnedUserId,
	},

	SearchLdap {
		user_id: OwnedUserId,
	},

	AuthLdap {
		user_dn: String,
		password: String,
	},
}
