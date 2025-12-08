Name:           pw-audioshare
Version:        1.0.1
Release:        1%{?dist}
Summary:        An accessible GTK4 patchbay for PipeWire

License:        MIT
URL:            https://github.com/destructatron/pw-audioshare
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  gtk4-devel >= 4.12
BuildRequires:  libadwaita-devel >= 1.4
BuildRequires:  pipewire-devel

Requires:       gtk4 >= 4.12
Requires:       libadwaita >= 1.4
Requires:       pipewire

%description
PW Audioshare is an accessible GTK4 patchbay for PipeWire. Unlike visual
node-graph tools like Helvum, PW Audioshare uses list-based views that
work well with screen readers like Orca.

Features:
- Connect and disconnect PipeWire audio, MIDI, and video ports
- Filter ports by type and search by name
- Bulk connect with multiple selection modes
- Save and load connection presets
- Full keyboard navigation
- Screen reader accessible

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release

%install
install -Dm755 target/release/%{name} %{buildroot}%{_bindir}/%{name}

%files
%license LICENSE
%doc README.md
%{_bindir}/%{name}

%changelog
* Mon Dec 08 2025 Harley Richardson <destructatron2018@gmail.com> - 1.0.1-1
- Add LICENSE file

* Mon Dec 08 2025 Harley Richardson <destructatron2018@gmail.com> - 1.0.0-1
- Initial release
