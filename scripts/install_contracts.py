"""Install the exact selected Contracts wheel after the locked dependency closure."""
import argparse
import json
import os
try:
    from scripts.environment import getenv, export_environment
except ModuleNotFoundError:
    from environment import getenv, export_environment

from pathlib import Path
import subprocess
import sys

from artifact_download import download_artifact
from artifact_paths import artifact_directory
from contracts_artifact import VERSION, require_new_contracts_environment, selected_wheel_details, verify_selected_install, verify_wheel


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest')
    args=parser.parse_args(argv)
    require_new_contracts_environment()
    if args.manifest or getenv('CIRNET_CONTRACTS_ARTIFACT_MANIFEST'):
        wheel,digest=selected_wheel_details(args.manifest)
    else:
        url=getenv('CIRNET_CONTRACTS_WHEEL_URL')
        digest=getenv('CIRNET_CONTRACTS_WHEEL_SHA256')
        if not url or not digest:
            parser.error('Select an immutable Contracts manifest or exact HTTPS URL/SHA256.')
        wheel=artifact_directory('hw-leaf-contracts')/f'cirnet_contracts-{VERSION}-py3-none-any.whl'
        wheel.write_bytes(download_artifact(url,digest))
        verify_wheel(wheel,digest)
    subprocess.run([sys.executable,'-m','pip','install','--no-deps','--force-reinstall',
        '--cache-dir',str(artifact_directory('hw-leaf-pip-cache')),str(wheel)],check=True)
    result=verify_selected_install(wheel,digest)
    if github_env := os.environ.get('GITHUB_ENV'):
        manifest=artifact_directory('hw-leaf-contracts')/'artifacts.json'
        manifest.write_text(json.dumps({'schemaVersion':1,'python':{'path':str(wheel),'sha256':digest.lower()}}),encoding='utf-8')
        with Path(github_env).open('a',encoding='utf-8') as output:
            output.write(f'CIRNET_CONTRACTS_ARTIFACT_MANIFEST={manifest}\n')
    print(result)


if __name__=='__main__':
    main()
