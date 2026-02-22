application_id = 'io.github.yusyel.ShelfilyDesktop'

scalable_dir = 'hicolor' / 'scalable' / 'apps'
install_data(
  scalable_dir / ('@0@.svg').format(application_id),
  install_dir: get_option('datadir') / 'icons' / scalable_dir
)

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
