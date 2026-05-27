application_id = 'io.github.yusyel.ShelfilyDesktop'

# NOTE: the scalable app SVG is intentionally NOT installed. It relies on
# <clipPath> + nested transforms that nanosvg-based renderers (e.g. the
# fuzzel launcher) cannot draw, leaving the icon blank. The fixed-size PNGs
# below (up to 512px) render correctly everywhere, including GNOME.

symbolic_dir = 'hicolor' / 'symbolic' / 'apps'
install_data(
  symbolic_dir / ('@0@-symbolic.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / symbolic_dir
)

foreach size_dir : ['64x64', '128x128', '256x256', '512x512']
  png_dir = 'hicolor' / size_dir / 'apps'
  install_data(
    png_dir / ('@0@.png').format(application_id),
    install_dir: get_option('datadir') / 'icons' / png_dir
  )
endforeach

configure_file(
  input: 'hicolor' / '512x512' / 'apps' / ('@0@.png').format(application_id),
  output: '@0@.png'.format(application_id),
  copy: true,
  install: true,
  install_dir: get_option('datadir') / 'pixmaps'
)
