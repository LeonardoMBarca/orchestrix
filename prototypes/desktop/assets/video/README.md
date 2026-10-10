# Vídeo de apresentação

A página `watch.html` está pronta para um vídeo HTML5, com controles nativos, sem autoplay, legendas WebVTT e transcrição opcional. `video-config.json` é o ponto de configuração; antes de preencher `src`, a página mostra o cartaz da paisagem e a disponibilidade do filme.

Exemplo após gravar o vídeo:

```json
{
  "src": "assets/video/orchestrix-walkthrough.mp4",
  "poster": "assets/visuals/night-flight.png",
  "captions": "assets/video/orchestrix-walkthrough.vtt",
  "transcript": "Texto da apresentação."
}
```

Copie o MP4/WebM e o VTT para esta pasta; use nomes com letras, números, hífen ou underscore. O servidor local disponibiliza somente esses tipos nessa pasta. URLs HTTP/HTTPS também são aceitas para mídia hospedada. `src` não é uma URL de página YouTube; use um arquivo de vídeo direto. Remova `captions` ou deixe vazio quando ainda não existir o arquivo. Legendas e transcrição devem corresponder ao vídeo que for publicado.

Não existe vídeo ou instalador real neste incremento. Não foi criada hospedagem ou publicação. A disponibilidade do vídeo no site não determina o estado dos gates do aplicativo.
