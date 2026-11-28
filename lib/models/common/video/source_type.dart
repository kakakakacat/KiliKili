import 'package:kilikili/services/rust/rust_audio.dart'
    show AudioPlaylistSource;

enum SourceType {
  normal(
    mediaType: -1,
    playlistSource: AudioPlaylistSource.upArchive,
  ),
  archive(
    mediaType: 1,
    extraId: 1,
    playlistSource: AudioPlaylistSource.mediaList,
  ),
  watchLater(
    mediaType: 2,
    extraId: 2,
    playlistSource: AudioPlaylistSource.mediaList,
  ),
  fav(
    mediaType: 3,
    extraId: 2,
    playlistSource: AudioPlaylistSource.userFavourite,
  ),
  playlist(
    mediaType: 3,
    extraId: 4,
    playlistSource: AudioPlaylistSource.mediaList,
  ),
  file,
  ;

  final int? mediaType;
  final int? extraId;
  final AudioPlaylistSource? playlistSource;
  const SourceType({
    this.mediaType,
    this.extraId,
    this.playlistSource,
  });
}
