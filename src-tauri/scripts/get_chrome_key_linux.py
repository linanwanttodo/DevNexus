#!/usr/bin/env python3
"""Get Chrome encryption key from libsecret via D-Bus.

Outputs the base64-encoded secret to stdout, or exits with code 1 on failure.

This is used by DevNexus on Linux to decrypt Chrome cookies.
Chrome stores its encryption key in the Secret Service (libsecret) with
attributes: xdg:schema=chrome_libsecret_os_crypt_password_v2, application=chrome.
"""
import sys
import dbus

def main():
    try:
        bus = dbus.SessionBus()
        secret_service = dbus.Interface(
            bus.get_object(
                'org.freedesktop.secrets',
                '/org/freedesktop/secrets',
                False
            ),
            'org.freedesktop.Secret.Service',
        )

        # Search for the Chrome secret with v2 schema
        object_paths = secret_service.SearchItems({
            'xdg:schema': 'chrome_libsecret_os_crypt_password_v2',
            'application': 'chrome',
        })

        result_paths = [
            p for p in object_paths
            if isinstance(p, dbus.Array) and len(p) > 0
        ]

        if not result_paths:
            # Try v1 schema
            object_paths = secret_service.SearchItems({
                'xdg:schema': 'chrome_libsecret_os_crypt_password_v1',
                'application': 'chrome',
            })
            result_paths = [
                p for p in object_paths
                if isinstance(p, dbus.Array) and len(p) > 0
            ]

        if not result_paths:
            sys.exit(1)

        obj_path = str(result_paths[0][0])

        # Unlock the collection
        secret_service.Unlock([obj_path])

        # Open a plain session (no DH exchange needed)
        _, session = secret_service.OpenSession(
            'plain', dbus.String('', variant_level=1)
        )

        # Get the secret
        secrets_dict = secret_service.GetSecrets([obj_path], session)
        secret_struct = secrets_dict[obj_path]
        # struct: (session, params, value, content_type)
        value_bytes = bytes(secret_struct[2])

        # Value is the base64-encoded key as raw bytes
        sys.stdout.buffer.write(value_bytes)
        sys.stdout.buffer.write(b'\n')

    except Exception:
        sys.exit(1)


if __name__ == '__main__':
    main()
