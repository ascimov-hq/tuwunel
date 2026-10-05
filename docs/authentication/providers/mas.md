# Matrix Authentication Service (MAS)

MAS attaches to Tuwunel as an upstream OpenID Connect identity provider.

## Which server is the issuer

**Tuwunel is its own next-generation-auth issuer and does not delegate
authentication to MAS.** This differs from the Synapse MSC3861 topology, where
MAS issues the access tokens the homeserver accepts. Tuwunel's built-in
[OIDC Authorization Server](../oidc-server.md) is what next-generation clients
authenticate against; MAS sits one layer further out, as one of the identity
providers that server can redirect the user to.

Two consequences worth stating plainly:

- Access tokens minted by MAS are unknown to Tuwunel. A client pointed at MAS as
  the homeserver's issuer authenticates successfully against MAS and then gets
  `401` from `/register`, `/room_keys/version` and every other authenticated
  endpoint.
- `mas_secret` does not make MAS the issuer. It authorizes MAS's provisioning
  calls on `/_synapse/mas/`, the counterpart of Synapse's shared secret. It does
  not delegate login or token issuance. Setting it also de-registers the
  Synapse-mirrored user-admin endpoints that MAS owns, matching Synapse's
  behavior.

Logging in through MAS therefore always needs a `[[global.identity_provider]]`
entry, with or without `mas_secret`.

## MAS configuration

Register Tuwunel as an OAuth client in your MAS configuration:

```yaml
clients:
  - client_id: 01J44Q10GR4AMTFZEEF936DTCM
    client_auth_method: client_secret_post
    client_secret: <client_secret>
    redirect_uris:
      - https://<your.matrix.example.com>/_matrix/client/unstable/login/sso/callback/01J44Q10GR4AMTFZEEF936DTCM
```

`client_secret_post` is required: Tuwunel sends `client_id` and `client_secret`
in the form-encoded body of the token request, not as HTTP basic credentials.

A statically configured MAS `client_id` must be a ULID, 26 characters of
Crockford base32; MAS rejects anything else. Use the same client ID in MAS's
`client_id`, Tuwunel's `client_id`, and the final path segment of both MAS's
redirect URI and Tuwunel's `callback_url`.

## Tuwunel configuration

```toml
[[global.identity_provider]]
brand = "MAS"
client_id = "01J44Q10GR4AMTFZEEF936DTCM"
client_secret = "<client_secret>"
issuer_url = "https://<your.mas.example.com>"
callback_url = "https://<your.matrix.example.com>/_matrix/client/unstable/login/sso/callback/01J44Q10GR4AMTFZEEF936DTCM"
```

Leave `authorization_url` unset. It overrides the authorization *endpoint*, not
the provider's base address, and discovery already fills it from MAS's
`.well-known/openid-configuration`. Pointing it at the MAS root sends users to
MAS's account-management page with the OAuth parameters dangling in the query
string, and the login never completes.

## Matching MAS-provisioned users

The example above leaves the provider untrusted, which is the safe default.

If `mas_secret` lets this same self-hosted MAS instance provision users, those
Matrix accounts already exist before their first SSO login. Add `trusted = true`
to this identity provider so MAS's `username` claim can match the provisioned
localpart:

```toml
trusted = true
```

Only enable this for the same MAS instance named by `issuer_url` when you
operate and fully control it. A trusted provider can authenticate as any
existing Matrix account whose localpart matches a returned username. The
`brand` value selects MAS-specific defaults; it does not establish trust or
link this provider to `mas_secret`. Never enable trusted mode for a public or
third-party MAS.

Leave `userid_claims` empty, or set it to exactly `["username"]`, so Tuwunel
considers the claim MAS uses for provisioning. Do not include `"sub"` in this
topology because it takes precedence over the username claim. If the provider
cannot be trusted, use an
[admin-approved association](../providers.md#admin-approved-association-for-untrusted-providers)
for each existing user instead.

## Scopes

For this login flow, MAS advertises the `openid` and `email` standard scopes in
its discovery document; its policy rejects `profile`. Tuwunel defaults `scope`
to `["openid"]` for `brand = "MAS"`, which is all MAS's userinfo endpoint needs:
it returns `sub` and `username`, and nothing further.

On Tuwunel 1.8.2 and earlier the default was `openid email profile`, which MAS
rejects. Set the scope explicitly on those versions:

```toml
scope = ["openid"]
```

## Environment variables

```env
TUWUNEL_IDENTITY_PROVIDER__0__BRAND="mas"
TUWUNEL_IDENTITY_PROVIDER__0__CLIENT_ID="01J44Q10GR4AMTFZEEF936DTCM"
TUWUNEL_IDENTITY_PROVIDER__0__CLIENT_SECRET="<client_secret>"
TUWUNEL_IDENTITY_PROVIDER__0__ISSUER_URL="https://<your.mas.example.com>"
TUWUNEL_IDENTITY_PROVIDER__0__CALLBACK_URL="https://<your.matrix.example.com>/_matrix/client/unstable/login/sso/callback/01J44Q10GR4AMTFZEEF936DTCM"
```

For the combined provisioning and login topology described above, the
equivalent setting is `TUWUNEL_IDENTITY_PROVIDER__0__TRUSTED="true"`.
