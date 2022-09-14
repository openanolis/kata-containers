set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly repo_root_dir="$(cd "${script_dir}/../../../.." && pwd)"
readonly kernel_builder="${repo_root_dir}/tools/packaging/kernel/build-kernel.sh"

DESTDIR=${DESTDIR:-${PWD}}
PREFIX=${PREFIX:-/opt/kata}
container_image="shim-v2-builder"

sudo docker build -t "${container_image}" "${script_dir}"

arch=$(uname -m)
if [ ${arch} = "ppc64le" ]; then
	arch="ppc64"
fi

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime-rs" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && make PREFIX=${PREFIX}"

sudo docker run --rm -i -v "${repo_root_dir}:${repo_root_dir}" \
	-w "${repo_root_dir}/src/runtime-rs" \
	"${container_image}" \
	bash -c "git config --global --add safe.directory ${repo_root_dir} && make PREFIX="${PREFIX}" DESTDIR="${DESTDIR}" install"

sudo sed -i -e '/^initrd =/d' "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-qemu.toml"
sudo sed -i -e '/^initrd =/d' "${DESTDIR}/${PREFIX}/share/defaults/kata-containers/configuration-dragonball.toml"

pushd "${DESTDIR}/${PREFIX}/share/defaults/kata-containers"
	sudo ln -sf "configuration-dragonball.toml" configuration.toml
popd