# Embedded Security

## Objective

Apply the existing Elvaris security model to unattended embedded devices on the
primary target.

## Requirements

Embedded images must use:

- Signed system images
- Signed application packages
- Protected trust roots
- Least-privilege services
- Read-only system files
- Device permissions
- Restricted debug access
- Protected recovery controls
- Secure update authorization
- Bounded logs
- No default unrestricted network service

## Debug policy

Separate policies for development, production, and recovery images. A
development serial shell must not automatically exist in production images.

## Secrets

Defined:

- Where device identity is stored
- Which services may access it
- How secrets are provisioned
- Whether secrets survive updates
- What happens on factory reset
- What is logged and what is not

Production private keys are not stored in the source tree or package manifest.

## Exit criteria

- [ ] Production and development policies differ
- [ ] Debug access is controlled
- [ ] Trust roots are protected
- [ ] Services use least privilege
- [ ] Device access is restricted
- [ ] System is read-only where intended
- [ ] Secrets are not logged
- [ ] Unauthorized recovery operations are denied
- [ ] Security tests pass
