import { ex, type Section } from "./types";

export const es: Section[] = [
  {
    id: "start",
    title: "Primeros pasos",
    html: `
<p>Renome renombra muchos archivos a la vez, en tres pasos:</p>
<ol>
  <li><b>Añada los archivos</b>: arrastre archivos o carpetas a la ventana, o use <b>+ Añadir</b>.</li>
  <li><b>Arme las reglas</b>: en <b>+ Regla</b>, elija qué hacer con los nombres (reemplazar texto, numerar, insertar una fecha…). La columna <b>Nuevo nombre</b> muestra el resultado al instante, antes de cambiar nada en el disco.</li>
  <li><b>Renombre</b>: haga clic en <b>Renombrar</b> (o <kbd>Ctrl</kbd>+<kbd>Enter</kbd>). Si algo sale mal, deshágalo en el <b>Historial</b>.</li>
</ol>
<p>Nada cambia en el disco hasta que haga clic en Renombrar, así que puede experimentar con libertad.</p>`,
  },
  {
    id: "files",
    title: "Lista de archivos",
    html: `
<h4>Añadir</h4>
<p>El botón <b>+ Añadir</b> tiene tres opciones:</p>
<ul>
  <li><b>Archivos…</b>: elija uno o varios archivos.</li>
  <li><b>Carpetas (con subcarpetas)…</b>: añade todos los archivos de dentro de las carpetas, incluidas las subcarpetas.</li>
  <li><b>Carpetas (renombrar las propias carpetas)…</b>: añade las carpetas en sí, para cambiar su nombre. Aparecen con un icono de carpeta.</li>
</ul>
<p>Arrastrar a la ventana funciona como "Archivos" y "Carpetas (con subcarpetas)". Los archivos que ya están en la lista se ignoran, y los recién añadidos quedan seleccionados.</p>
<p>Los archivos ocultos (que empiezan con punto, como <code>.config</code>, y, en Windows, los marcados como ocultos o de sistema, como <code>desktop.ini</code> y <code>Thumbs.db</code>) quedan fuera al añadir carpetas, salvo que active la opción correspondiente. Una carpeta oculta que usted mismo añade entra con todo lo que tiene.</p>
<p>También puede abrir el programa con archivos: <code>renome foto1.jpg foto2.jpg carpeta/</code>.</p>

<h4>Filtrar</h4>
<p>El campo <b>Filtrar</b> muestra solo los archivos que coinciden. <b>Solo se renombran los archivos visibles</b>, y la numeración cuenta solo esos.</p>
<ul>
  <li>Términos separados por comas o espacios: <code>jpg, vacaciones</code> muestra los nombres que contienen "jpg" <i>o</i> "vacaciones".</li>
  <li><code>*</code> y <code>?</code> son comodines: <code>*.png</code> (termina en .png), <code>IMG_????.jpg</code> (cuatro caracteres cualesquiera).</li>
  <li>Entre comillas, busca la frase completa: <code>"breaking bad"</code>.</li>
  <li>Con <code>-</code> delante, oculta lo que coincide: <code>-*.srt</code> oculta los subtítulos; <code>*.mkv -sample</code> muestra los .mkv, salvo las muestras.</li>
  <li>Mayúsculas y acentos no importan: <code>cancion</code> encuentra "Canción".</li>
</ul>

<h4>Ordenar y organizar</h4>
<ul>
  <li>Haga clic en el título de una columna para ordenar; otra vez para invertir.</li>
  <li>El orden predeterminado es por <b>Nuevo nombre</b>: la lista se reordena mientras cambia las reglas.</li>
  <li>Seleccione con clic, <kbd>Ctrl</kbd>+clic (varios) y <kbd>Mayús</kbd>+clic (un intervalo).</li>
  <li><b>⤒ ⤓</b> mueven los seleccionados hacia arriba o abajo, útil para ajustar la numeración. No funcionan con el orden por Nuevo nombre.</li>
  <li><b>Quitar</b> (o <kbd>Supr</kbd>) saca los seleccionados de la lista; <b>Limpiar</b> la vacía. Los archivos del disco no se tocan. Lo que las búsquedas encontraron para ellos también sale, y un archivo añadido de nuevo empieza de cero (<kbd>Ctrl</kbd>+<kbd>Z</kbd> lo trae todo de vuelta).</li>
  <li><kbd>Ctrl</kbd>+<kbd>Z</kbd> deshace el último cambio en la lista.</li>
</ul>

<h4>Columnas de información</h4>
<p>El botón <b>☰</b>, en el extremo izquierdo del encabezado de la lista, aparece según las reglas y activa columnas con lo que guardan los archivos: con la regla de música, <b>Artista</b>, <b>Álbum</b>, <b>Título</b>, <b>Pista</b>, <b>Año</b>, <b>Género</b>, <b>Duración</b> y <b>Formato</b> (como <code>MP3 · 320 kbps</code> o <code>FLAC · 24/96</code>); con la de película o serie, <b>Resolución</b> y <b>Códec</b>; con la regla Insertar usando la fecha en que se tomó la foto, <b>Fecha de la foto</b>. Sin ninguna de estas, el botón no aparece. La elección queda guardada y vuelve con la regla. Haga clic en el título de una para ordenar la lista: al ordenar por artista o año, cada álbum queda en orden de disco y pista. Los archivos sin la información van al final. <b>Exportar lista</b> incluye también las columnas activadas.</p>
<p><b>Limitaciones:</b> la información se lee del propio archivo, solo mientras la columna está activada; en carpetas grandes o discos lentos, ordenar por una de estas columnas la primera vez tarda un poco, porque hay que leer cada archivo. La resolución y el códec solo salen de los formatos de vídeo que Renome lee (MKV, WebM, MP4, MOV, AVI, MPEG y FLV), y la fecha de la foto solo de fotos con datos EXIF. Con muchas columnas activadas en una ventana estrecha, la lista se desplaza hacia los lados.</p>`,
  },
  {
    id: "rules",
    title: "Cómo funcionan las reglas",
    html: `
<p>Las reglas se aplican <b>de arriba abajo</b>, cada una sobre el resultado de la anterior. El orden importa: numerar y luego cambiar mayúsculas no da lo mismo que al revés.</p>
<ul>
  <li>La <b>casilla</b> del título activa y desactiva la regla sin borrarla. Desactivada, queda bloqueada: para cambiar sus opciones o buscar, actívela antes.</li>
  <li><b>↑ ↓</b> cambian el orden; <b>✕</b> quita la regla; un clic en el título pliega la tarjeta.</li>
  <li>Una regla con error (una expresión inválida, por ejemplo) queda con borde rojo y se ignora hasta que se corrija.</li>
  <li><b>Una regla de búsqueda a la vez</b>: película, serie, música, libro y cómic no van juntas. Con una de ellas en la lista, las demás quedan desactivadas en <b>+ Regla</b> hasta que se quite (desactivarla no basta). Las demás reglas (reemplazar, numerar, insertar…) combinan con cualquiera. Un conjunto de reglas guardado antes con más de una se abre solo con la primera, y un aviso dice cuáles quedaron fuera. Para una carpeta con películas y series mezcladas, renombre en dos pasadas.</li>
  <li>Una búsqueda en internet (película, serie, música, libro, cómic) se puede detener con <b>Detener</b>, en el cuadro de progreso. Lo encontrado se queda, y buscar de nuevo sigue con el resto. La consulta en curso termina antes, lo que puede tardar unos segundos.</li>
  <li><b>Pegar un enlace</b>: cuando la búsqueda por nombre no encuentra el correcto, pegue en el campo de búsqueda de la ventana de elección el enlace de la página en el catálogo, y viene exactamente ese elemento. Película: enlace de TMDB o IMDb, o el código <code>tt…</code>. Serie: ID o enlace de TMDB, enlace de IMDb, o ID/enlace de AniList. Libro: un ISBN o el enlace de Open Library. Álbum (en el campo del álbum): enlace del lanzamiento en MusicBrainz o Discogs. Cómic: enlace de la serie en el catálogo elegido en la regla (Comic Vine, también de un número; GCD; MangaDex; AniList; MangaUpdates). Limitaciones: en Metron solo sirve la dirección con el número de la serie (no la del nombre); un enlace de un catálogo distinto al de la regla se rechaza; un número solo se lee como título (solo en la serie vale como ID).</li>
</ul>
<h4>Aplicar a</h4>
<p>Varias reglas preguntan dónde actuar:</p>
<ul>
  <li><b>Nombre (sin extensión)</b>: lo predeterminado. En <code>foto.jpg</code>, actúa solo sobre <code>foto</code>.</li>
  <li><b>Extensión</b>: solo sobre <code>jpg</code>.</li>
  <li><b>Nombre completo</b>: sobre <code>foto.jpg</code> entero.</li>
</ul>
<p>Las carpetas no tienen extensión: en <code>Serie.2020</code>, el <code>.2020</code> es parte del nombre.</p>
<h4>Guardar y abrir reglas</h4>
<p>📂 abre y 💾 guarda el conjunto de reglas en un archivo <code>.json</code>, para reutilizarlo. Por defecto, el programa también recuerda las reglas al cerrarse.</p>
<h4>Posiciones</h4>
<p>Cuando una regla pide una posición, cuenta caracteres visibles desde 1. Las letras con tilde y los emojis cuentan como un carácter.</p>`,
  },
  {
    id: "r-replace",
    title: "Reemplazar texto",
    sub: true,
    html: `
<p>Cambia un texto por otro.</p>
${ex("foto_de_vacaciones.jpg", "foto de vacaciones.jpg")}
<ul>
  <li><b>Buscar</b> y <b>Reemplazar por</b>: deje "Reemplazar por" vacío para borrar el texto.</li>
  <li><b>Distinguir mayúsculas</b>: sin marcar, "Foto" también encuentra "foto" y "FOTO".</li>
  <li><b>Solo la 1ª aparición en cada nombre</b>: cambia solo la primera vez que aparece el texto <i>en cada nombre</i>. En <code>a_b_c</code>, cambiar <code>_</code> por un espacio da <code>a b_c</code>. Si el texto aparece una sola vez, todos los nombres cambian igual.</li>
</ul>`,
  },
  {
    id: "r-regex",
    title: "Expresión regular",
    sub: true,
    html: `
<p>Para patrones que "Reemplazar texto" no alcanza. Los grupos entre paréntesis se reutilizan en el reemplazo como <code>$1</code>, <code>$2</code>…</p>
${ex("01 - Apertura.mp3", "Apertura (01).mp3")}
<p>Expresión <code>(\\d+) - (.*)</code>, reemplazar por <code>$2 ($1)</code>.</p>
<table>
  <tr><td><code>.</code></td><td>cualquier carácter</td></tr>
  <tr><td><code>\\d</code> <code>\\s</code> <code>\\w</code></td><td>dígito, espacio, letra/dígito/_</td></tr>
  <tr><td><code>*</code> <code>+</code> <code>?</code></td><td>cero o más, uno o más, opcional</td></tr>
  <tr><td><code>{2}</code> <code>{1,3}</code></td><td>exactamente 2, de 1 a 3 veces</td></tr>
  <tr><td><code>[abc]</code> <code>[^abc]</code></td><td>uno de esos caracteres, cualquiera menos esos</td></tr>
  <tr><td><code>^</code> <code>$</code></td><td>inicio y fin del texto</td></tr>
  <tr><td><code>( )</code></td><td>grupo, reutilizado como <code>$1</code>, <code>$2</code>…</td></tr>
  <tr><td><code>\\.</code> <code>\\(</code></td><td>el propio punto, el propio paréntesis</td></tr>
</table>
<p class="tip">Si un grupo va seguido de letras o dígitos, use llaves: <code>\${1}a</code>. Escrito <code>$1a</code>, el programa busca un grupo llamado "1a", que no existe.</p>
<p>También funcionan los grupos con nombre: <code>(?P&lt;anio&gt;\\d{4})</code> y luego <code>\${anio}</code>.</p>`,
  },
  {
    id: "r-rearrange",
    title: "Reorganizar partes",
    sub: true,
    html: `
<p>Divide el nombre en un separador y lo arma de nuevo en otro orden. No hacen falta expresiones regulares.</p>
${ex("Serie - Episodio - S01E02.mkv", "Serie - S01E02 - Episodio.mkv")}
<ul>
  <li><b>Separador</b>: el texto entre las partes (aquí, <code> - </code>). Se quitan los espacios alrededor de cada parte y se descartan las partes vacías.</li>
  <li>La tarjeta muestra las partes del primer archivo de la lista, numeradas. <b>Haga clic en una parte</b> para añadirla al nuevo orden.</li>
  <li><b>Nuevo orden</b>: los números son las partes; todo lo demás se mantiene como texto. Para el ejemplo: <code>1 - 3 - 2</code>.</li>
  <li>Las partes que no aparecen en el orden se eliminan. El orden <code>2 (1)</code> en <code>García - Ana</code> da <code>Ana (García)</code>.</li>
  <li>Si un nombre tiene menos partes de las que pide el orden, queda como está.</li>
</ul>`,
  },
  {
    id: "r-case",
    title: "Mayúsculas / minúsculas",
    sub: true,
    html: `
<ul>
  <li><b>minúsculas</b>: <code>Foto De La Playa</code> → <code>foto de la playa</code></li>
  <li><b>MAYÚSCULAS</b>: → <code>FOTO DE LA PLAYA</code></li>
  <li><b>Primera letra mayúscula</b>: → <code>Foto de la playa</code></li>
  <li><b>Cada Palabra En Mayúscula</b>: → <code>Foto De La Playa</code></li>
  <li><b>Cada Palabra, excepto de/la/y/of…</b>: → <code>Foto de la Playa</code>. Las palabras cortas de enlace (de, del, la, los, y, con, of, the, and, da, do…) quedan en minúsculas, salvo al inicio del nombre o de una parte (después de <code> - </code>, <code>:</code> o <code>(</code>).</li>
</ul>
<p>En los dos modos "Cada Palabra", los números romanos quedan en mayúsculas (<code>Fase III</code>, <code>Parte II</code>) y las palabras con mayúsculas en medio quedan como están (<code>iPhone</code>, <code>McDonald's</code>).</p>
<p>Use "Aplicar a: Extensión" para cambiar solo la extensión.</p>`,
  },
  {
    id: "r-insert",
    title: "Insertar",
    sub: true,
    html: `
<p>Añade algo al nombre.</p>
<h4>Qué</h4>
<ul>
  <li><b>Texto</b>: un texto fijo.</li>
  <li><b>Fecha actual</b>: la fecha de hoy.</li>
  <li><b>Fecha de modificación del archivo</b>.</li>
  <li><b>Fecha en que se tomó la foto</b>: leída de los datos EXIF de la foto (JPEG, HEIC, PNG, WebP, TIFF y formatos RAW). Las fotos sin esa fecha quedan como están, salvo que <b>"Sin esa fecha, usar la de modificación"</b> esté marcado. Los vídeos aún no se leen.</li>
  <li><b>Nombre de la carpeta</b>: el nombre de la carpeta donde está el archivo.</li>
</ul>
${ex("IMG_1234.jpg", "2024-07-15 IMG_1234.jpg")}
<h4>Formato de fecha</h4>
<table>
  <tr><td><code>%Y</code> <code>%y</code></td><td>año con 4 o 2 dígitos (2024, 24)</td></tr>
  <tr><td><code>%m</code> <code>%d</code></td><td>mes y día (07, 15)</td></tr>
  <tr><td><code>%H</code> <code>%M</code> <code>%S</code></td><td>hora, minuto, segundo</td></tr>
  <tr><td><code>%j</code></td><td>día del año (001–366)</td></tr>
</table>
<p>Ejemplo: <code>%Y-%m-%d %H.%M.%S</code> da <code>2024-07-15 14.32.10</code>. Una <code>/</code> en el formato se convierte en <code>-</code>, porque los nombres de archivo no admiten barras.</p>
<h4>Dónde</h4>
<ul>
  <li><b>Al inicio</b> o <b>Al final</b> del nombre (antes de la extensión).</li>
  <li><b>En la posición</b> N: antes del N-ésimo carácter (1 = al inicio).</li>
  <li><b>En la posición (contando desde el final)</b> N: N caracteres antes del final (0 = al final, 1 = antes del último).</li>
</ul>`,
  },
  {
    id: "r-number",
    title: "Numerar",
    sub: true,
    html: `
<p>Numera los archivos en el orden de la lista.</p>
${ex("playa.jpg", "Viaje 01.jpg")}
<ul>
  <li><b>Plantilla</b>: el nuevo nombre, con variables:
    <ul>
      <li><code>{n}</code>: el número;</li>
      <li><code>{nombre}</code>: el nombre actual (sin extensión);</li>
      <li><code>{carpeta}</code>: el nombre de la carpeta donde está el archivo.</li>
    </ul>
    Ejemplos: <code>Viaje {n}</code>, <code>{nombre} - {n}</code>, <code>{carpeta} {n}</code>.</li>
  <li><b>Inicio</b> y <b>Paso</b>: 1 y 1 dan 1, 2, 3…; 10 y 10 dan 10, 20, 30…; un paso negativo cuenta hacia atrás.</li>
  <li><b>Dígitos</b>: completa con ceros a la izquierda (2 dígitos: 01, 02… 10).</li>
  <li><b>Reiniciar la numeración en cada carpeta</b>: cada carpeta empieza de nuevo.</li>
</ul>
<p>La numeración sigue el orden de la lista y cuenta solo los archivos visibles en el filtro. Con el orden por Nuevo nombre, sigue el orden del nombre actual. Para controlar el orden, ordene por otra columna o use ⤒ ⤓.</p>`,
  },
  {
    id: "r-remove",
    title: "Quitar caracteres",
    sub: true,
    html: `
<ul>
  <li><b>Espacios repetidos y en los extremos</b>: <code>  foto   de  la playa </code> → <code>foto de la playa</code></li>
  <li><b>Todos los espacios</b>: → <code>fotodelaplaya</code></li>
  <li><b>Acentos</b>: <code>Canción</code> → <code>Cancion</code>; también letras especiales, como <code>Ørsted</code> → <code>Orsted</code>, <code>Straße</code> → <code>Strasse</code>, <code>Łódź</code> → <code>Lodz</code></li>
  <li><b>Dígitos</b>: <code>foto123</code> → <code>foto</code></li>
  <li><b>Número al inicio</b>: el número de pista al comienzo del nombre y el separador que lo sigue: <code>01 - Canción</code>, <code>01. Canción</code> y <code>1-05 Canción</code> pasan a <code>Canción</code>. Números de hasta tres dígitos: los años (<code>1999 Viaje</code>) y las fechas (<code>2024-07-15 Playa</code>) quedan, y también un nombre que es solo el número.</li>
  <li><b>Caracteres</b>: cada carácter escrito se quita donde aparezca. <code>_#</code> quita todos los <code>_</code> y <code>#</code>.</li>
  <li><b>Limitar a</b> N caracteres: corta el nombre (sin la extensión) después de N caracteres, quitando los espacios y guiones que sobren al final. Útil para radios de coche, memorias USB y programas antiguos que no aceptan nombres largos. Con 0 no hay límite. Cuidado: dos nombres largos que solo cambian al final pueden quedar iguales, y el conflicto aparece en la lista.</li>
</ul>`,
  },
  {
    id: "r-convert",
    title: "Convertir caracteres",
    sub: true,
    html: `
<p>Cambia caracteres por otros, sin borrar nada.</p>
<ul>
  <li><b>Arreglar acentos rotos</b>: los nombres creados en un sistema y leídos con otra codificación quedan con los acentos cambiados por pares de signos: <code>CoraÃ§Ã£o</code>, <code>SÃ£o JoÃ£o</code>, <code>donâ€™t</code>. La opción devuelve el texto correcto: <code>Coração</code>, <code>São João</code>, <code>don’t</code>. Un nombre que ya está bien queda como está.</li>
  <li><b>Otros alfabetos en letras latinas</b>: el cirílico (ruso, ucraniano, serbio…) y el griego pasan a letras latinas, como se pronuncian: <code>Кино</code> → <code>Kino</code>, <code>Щедрик</code> → <code>Shchedrik</code>, <code>Ελλάδα</code> → <code>Ellada</code>. El resto del nombre queda igual. El japonés, el chino, el árabe y otros alfabetos no se convierten.</li>
  <li><b>Nombre seguro para internet</b>: deja solo letras sin acento, números y puntos, con <code>-</code> o <code>_</code> entre las palabras: <code>Férias em São Paulo (2024)</code> → <code>Ferias-em-Sao-Paulo-2024</code>. Sirve para enviar archivos a sitios web, servidores FTP y aparatos antiguos. También convierte el cirílico y el griego; las letras de otros alfabetos salen del nombre. Si no queda nada, el nombre queda vacío y la lista muestra el error.</li>
</ul>
<p><b>Limitaciones</b>: solo arregla el defecto más común (texto en UTF-8 leído como Latin-1 o Windows-1252). Los nombres cuyos acentos se volvieron <code>?</code> o <code>�</code> perdieron la letra para siempre y no tienen arreglo. Un nombre que mezcla letras correctas y rotas en la misma palabra puede quedar como está.</p>
<p>Para arreglar nombres sacados de las etiquetas, ponga esta regla debajo de la regla Música (etiquetas).</p>`,
  },
  {
    id: "r-range",
    title: "Quitar fragmento",
    sub: true,
    html: `
<p>Borra un trozo del nombre, definido por dónde empieza y dónde termina.</p>
<ul>
  <li><b>Desde</b>: el inicio del nombre, una posición, o la primera vez que aparece un texto.</li>
  <li><b>Hasta</b>: el final del nombre, una posición, o un texto (buscado después del inicio).</li>
  <li><b>Quitar también los textos delimitadores</b>: si los propios textos de inicio y fin también se van.</li>
</ul>
${ex("Película [1080p] (2020).mkv", "Película  (2020).mkv")}
<p>Desde el texto <code>[</code> hasta el texto <code>]</code>, quitando los delimitadores. Los textos distinguen mayúsculas. Si alguno no aparece en un nombre, ese nombre queda como está.</p>`,
  },
  {
    id: "r-char",
    title: "Carácter en posición",
    sub: true,
    html: `
<p>Cambia o borra el carácter de una posición.</p>
<ul>
  <li><b>Posición</b>: desde 1. Con <b>Contar desde el final</b>, 1 es el último carácter.</li>
  <li><b>Solo si es</b>: solo lo cambia si el carácter de esa posición es este (vacío = cualquiera). No distingue mayúsculas, salvo que <b>Distinguir mayúsculas</b> esté marcado.</li>
  <li><b>Cambiar por</b>: el nuevo carácter. Vacío lo borra.</li>
</ul>
${ex("_foto.jpg", "foto.jpg")}
<p>Posición 1, solo si es <code>_</code>, cambiar por nada.</p>`,
  },
  {
    id: "r-ext",
    title: "Extensión",
    sub: true,
    html: `
<ul>
  <li><b>minúsculas</b> / <b>MAYÚSCULAS</b>: <code>foto.JPG</code> → <code>foto.jpg</code></li>
  <li><b>Definir</b>: cambia la extensión por otra (<code>jpeg</code> → <code>jpg</code>). No convierte el archivo, solo cambia el nombre.</li>
  <li><b>Quitar</b>: elimina la extensión.</li>
  <li><b>Por el contenido</b>: mira los primeros bytes de cada archivo y le da la extensión de lo que es en realidad. Un <code>.jpg</code> que es PNG pasa a <code>.png</code>; un <code>.cbz</code> que es RAR pasa a <code>.cbr</code> (y un <code>.rar</code> con solo imágenes pasa a <code>.cbz</code> cuando es zip); un archivo sin extensión recibe una. Las extensiones que ya sirven para ese contenido quedan: un RAW de cámara (<code>.nef</code>, <code>.cr2</code>) es TIFF por dentro y no pasa a <code>.tif</code>; un <code>.m4b</code> no pasa a <code>.m4a</code>; las MAYÚSCULAS siguen en MAYÚSCULAS. En un nombre como <code>programa_1.0_x86[0]</code>, lo que sigue al último punto no es extensión, así que la nueva se añade al final. Cuando el nombre ya trae, antes de la extensión equivocada, una que sirve para el contenido, la equivocada solo se quita: <code>pelicula.nfo.pdf</code> que es texto pasa a <code>pelicula.nfo</code>, y <code>foto.png.jpg</code> que es PNG pasa a <code>foto.png</code>. La extensión de dentro solo queda si el archivo es de verdad de ese tipo: <code>datos.json.pdf</code> solo pasa a <code>datos.json</code> si el contenido es JSON válido, y lo mismo vale para XML, HTML, subtítulos y listas; los formatos de texto libre (<code>.txt</code>, <code>.nfo</code>, <code>.csv</code>, <code>.log</code>) aceptan cualquier texto.</li>
</ul>
<p>El texto se trata aparte: un archivo de texto solo cambia cuando su nombre dice que es de un formato que no es texto (<code>.pdf</code>, <code>.jpg</code>, <code>.mp3</code>…). Entonces pasa a <code>.txt</code>, o a <code>.html</code>, <code>.xml</code>, <code>.svg</code>, <code>.json</code>, <code>.srt</code>, <code>.vtt</code>, <code>.ass</code> o <code>.m3u8</code> cuando su comienzo dice cuál es. Un <code>.csv</code>, <code>.md</code>, <code>.log</code> o un archivo de texto sin extensión (<code>README</code>) queda como está.</p>
<p><b>Limitaciones:</b> reconoce imágenes, audio, vídeo, PDF, libros (EPUB, MOBI, FB2, DjVu), archivos comprimidos, documentos de Office y LibreOffice, fuentes y texto. Los documentos antiguos de Office (<code>.doc</code>, <code>.xls</code>), los programas y las bases de datos no se reconocen y quedan como están. Entre los textos, solo se distinguen los tipos de arriba: un CSV llamado <code>.pdf</code> pasa a <code>.txt</code>. No convierte nada: solo corrige el nombre.</p>
<p>Esta regla no afecta a las carpetas.</p>`,
  },
  {
    id: "r-movie",
    title: "Película (TMDB)",
    sub: true,
    html: `
<p>Da a cada película su nombre oficial de <b>TMDB</b> (themoviedb.org), en el formato que reconoce Jellyfin. Necesita una <b>clave de la API de TMDB</b>, gratuita, introducida en Opciones.</p>
${ex("Nombre.de.la.Pelicula.2001.1080p.BluRay.x264.mkv", "Nombre de la Película (2001) [tmdbid-12345].mkv")}
<div class="notice">
<p><b>Antes de usarla: el nombre de los archivos importa</b></p>
<p>La película se busca por el <b>título</b> y el <b>año</b> que hay en el nombre del archivo. Los nombres llenos de marcas de lanzamiento (<code>Nombre.de.la.Pelicula.2001.1080p.BluRay</code>) funcionan bien, porque las marcas se descartan. Pero un nombre sin el título (<code>pelicula1.mkv</code>), solo con siglas o abreviaturas, o con el título en un idioma que TMDB no tiene, no se encuentra o va a la revisión; sin el año, las películas con el mismo nombre se confunden.</p>
<p>Si sus archivos están lejos de eso, ponga encima de esta regla, en la lista, las reglas que arreglan el nombre (<b>Reemplazar texto</b>, <b>Quitar fragmento</b>, <b>Expresión regular</b>): la búsqueda lee el nombre tal como sale de ellas, y la columna <b>Nuevo nombre</b> muestra el resultado antes de renombrar.</p>
</div>
<ol>
  <li>Añada la regla y haga clic en <b>Buscar en TMDB</b>. El programa toma el título y el año del nombre de cada archivo (los puntos pasan a espacios; se quitan corchetes, paréntesis y marcas como 1080p, BluRay, x264) y busca en TMDB.</li>
  <li>Cuando título y año coinciden sin duda, la película se elige sola (<b>✓</b> junto al nuevo nombre). Si hay dudas (<b>?</b>), se abre la ventana de revisión: haga clic en la película correcta (o pulse 1–9). El año del archivo puede diferir en uno del de TMDB; se tiene en cuenta.</li>
  <li>Si ninguna sirve, corrija el título o el año en el campo de búsqueda de la ventana, o elija <b>Ninguna de estas</b> (<b>✕</b>: el archivo queda como está).</li>
</ol>
<ul>
  <li><b>Plantilla</b>: <code>{título}</code>, <code>{original}</code> (título original), <code>{año}</code> e <code>{id}</code>. La predeterminada es <code>{título} ({año}) [tmdbid-{id}]</code>.</li>
  <li>También, leídos desde dentro del vídeo: <code>{resolución}</code> (<code>2160p</code>, <code>1080p</code>, <code>720p</code>, <code>576p</code>, <code>480p</code>; una película recortada más ancha, como 1920×800, sigue siendo <code>1080p</code>) y <code>{códec de vídeo}</code> (<code>H.264</code>, <code>HEVC</code>, <code>AV1</code>, <code>VP9</code>, <code>MPEG-4</code>). Se leen de MKV, WebM, MP4, MOV, M4V, AVI, MPG, VOB y FLV (por su contenido, incluso con la extensión equivocada), sin leer el vídeo entero, y también de archivos descargados a medias; en otros formatos (TS, WMV, RealMedia, ISO) quedan vacíos. Los subtítulos reciben los de su vídeo. Úselos en una parte opcional: <code>{título} ({año})&lt; - {resolución}&gt;</code> da <code>Película (2010) - 1080p.mkv</code>, el nombre que Jellyfin usa para guardar varias versiones de la misma película (con <b>Crear una carpeta para cada película</b>, las versiones quedan juntas en una carpeta sin la resolución en el nombre).</li>
  <li><b>Más variables</b>, de TMDB: <code>{género}</code>, <code>{géneros}</code> (hasta tres), <code>{país}</code>, <code>{idioma}</code> (el original, como <code>en</code>), <code>{duración}</code> (en minutos), <code>{clasificación}</code> (la por edades del país del idioma de los títulos: <code>es-ES</code> da la de España), <code>{director}</code>, <code>{estudio}</code>, <code>{colección}</code> y la fecha de estreno. Fechas: <code>{fecha}</code> (<code>2010-05-03</code>), <code>{mes}</code> (<code>05</code>), <code>{día}</code> y el nombre del mes en el idioma de la variable: <code>{nombre del mes}</code> da <code>mayo</code>, <code>{month name}</code> da <code>May</code> y <code>{nome do mês}</code> da <code>maio</code>. Del nombre del archivo: <code>{edición}</code> (Director's Cut, Extended, Unrated, IMAX…), <code>{fuente}</code> (BluRay, Remux, WEB-DL, WEBRip, HDTV, DVD…) y <code>{grupo}</code> (el grupo de lanzamiento: <code>-GRUPO</code> al final de un nombre con fuente o resolución). Limitación: las de TMDB llegan justo después de la búsqueda (una consulta más por película), y país y estudio vienen como TMDB los escribe, en general en inglés.</li>
  <li><b>Lo que ya está al lado</b>: si el vídeo tiene un <code>.nfo</code> con el mismo nombre (o la carpeta tiene un <code>movie.nfo</code>), como los que graban Renome, Jellyfin, Kodi y Emby, su ID de TMDB dice qué película es, sin buscar por el nombre. Con solo el ID de IMDb, o un enlace de TMDB o IMDb en el texto (como en las notas de un lanzamiento), se consulta TMDB por él.</li>
  <li><b>Partes opcionales</b>: una parte entre <code>&lt; &gt;</code> solo aparece cuando todas las variables dentro de ella tienen valor (los signos <code>&lt; &gt;</code> no van al nombre). <code>&lt;{año} - &gt;{título}</code> da <code>2010 - Título</code>, o solo <code>Título</code> cuando no se conoce el año.</li>
  <li><b>Alternativa</b>: <code>{a|b}</code> pone la primera de esas variables que tenga valor. <code>{título|año}</code> da el título, o el año cuando no hay título. Puede tener más de dos (<code>{a|b|c}</code>) y funciona dentro de una parte opcional.</li>
  <li><b>Cifras</b>: <code>{variable:N}</code> escribe los números con N cifras. <code>{a:3}</code> da <code>005</code> en vez de <code>05</code>, y <code>{a:1}</code> da <code>5</code>, sin ceros delante. Los decimales (<code>12.1</code>) y el disco antes de la pista (<code>1-05</code>) quedan como están; el texto sin número no cambia.</li>
  <li><b>Cambiar ":" por</b>: Windows no acepta dos puntos; por defecto pasan a <code> - </code>. Las barras pasan a guiones y se quitan <code>? * &lt; &gt; |</code>.</li>
  <li>Solo se buscan y renombran vídeos, subtítulos y carpetas; las imágenes, los <code>.nfo</code> y otros archivos de la lista conservan su nombre, salvo los que tienen el nombre de la película o van con ella a la carpeta nueva (vea abajo). Los subtítulos con el mismo nombre que la película reciben la misma elección. Los subtítulos conservan el idioma y las marcas <b>forced</b> (solo las líneas en otro idioma), <b>sdh</b>/<b>cc</b> (para sordos) y <b>foreign</b> al final del nombre, como los leen Jellyfin, Plex y Kodi: <code>Nombre de la Película (1999) [tmdbid-12345].pt-BR.forced.srt</code>. Una marca escrita con guion, guion bajo o espacio (<code>Pelicula-forced.srt</code>) pasa a <code>.forced</code>.</li>
  <li><b>Crear una carpeta para cada película</b>: la película y sus subtítulos van a una carpeta con el nuevo nombre, como recomienda Jellyfin: <code>Películas/Nombre de la Película (1999) [tmdbid-12345]/Nombre de la Película (1999) [tmdbid-12345].mkv</code>. Una película que ya está en la carpeta correcta no se mueve de nuevo. Si está en una carpeta con su nombre en otro formato (<code>Nombre.de.la.Pelicula.1999.1080p</code>), o en una carpeta de cualquier nombre que solo tenga la película y lo que la acompaña (subtítulos, imágenes, <code>.nfo</code>, <code>.txt</code>), la carpeta nueva queda junto a ella, no dentro (vea <b>Carpetas antiguas que se quedan con archivos</b>, en Vista previa y conflictos). Deshacer devuelve los archivos y borra la carpeta si queda vacía.</li>
  <li>Lo que un servidor multimedia dejó en la carpeta de la película (<code>backdrop.jpg</code>, <code>poster.jpg</code>, <code>movie.nfo</code>, <code>extrafanart/</code>) va con ella a la carpeta nueva, también al enviar a otra carpeta. Los archivos con el nombre de la película (<code>Nombre.nfo</code>, <code>Nombre-poster.jpg</code>) reciben el nombre nuevo. Limitación: solo los archivos que están en la lista y solo de una carpeta que es de la película (con su nombre, o solo con ella y lo que acompaña a una película); en una carpeta con otras cosas, solo van los archivos con el nombre de la película.</li>
  <li>Para quitar las tildes, añada la regla <b>Quitar caracteres → Acentos</b> después de esta.</li>
</ul>
<h4>Metadatos (.nfo y póster)</h4>
<p>Con <b>Guardar .nfo y póster junto a la película</b>, al renombrar se crean, junto a cada vídeo, <code>Nombre (año) [tmdbid-…].nfo</code> y el póster; opcionalmente también la imagen de fondo (fanart) y el logo de la película. Dentro de la carpeta de la película, las imágenes se llaman <code>poster.jpg</code>, <code>fanart.jpg</code> y <code>logo.png</code>; sueltas, llevan delante el nombre de la película (<code>…-poster.jpg</code>). Jellyfin, Kodi y Emby leen estos archivos.</p>
<p>El <code>.nfo</code> incluye todo lo que TMDB tiene y Jellyfin lee: título y título original, año y fecha de estreno, sinopsis y eslogan, duración, nota y votos, la clasificación por edades del país del idioma elegido (ej.: ES-12), géneros, palabras clave (como etiquetas), países, estudios, colección (ej.: "Nombre de la Colección"), tráiler de YouTube, dirección, guion, reparto con personajes y fotos, productores, compositor y los ids de TMDB e IMDb. El archivo de la película no cambia, los archivos que ya existen no se reemplazan y <b>deshacer</b> el renombrado borra lo creado.</p>
<p>Las películas que <b>ya tienen el nombre correcto</b> también reciben los metadatos: el botón Renombrar las cuenta y la confirmación lo avisa. Si el nombre ya trae <code>[tmdbid-…]</code>, la película se reconoce directamente por ese id, sin buscar. Así una biblioteca ya organizada puede recibir solo los .nfo y las imágenes.</p>
<h4>Subtítulos</h4>
<p>Con <b>Descargar subtítulos</b>, el programa busca subtítulos para las películas ya identificadas en <b>OpenSubtitles.com</b> y <b>SubDL</b> y descarga los elegidos <b>al renombrar</b>, ya con el nombre nuevo del vídeo, como los leen Jellyfin, Plex y Kodi: <code>Nombre de la Película (1999).es.srt</code> (<code>.sdh.srt</code> cuando es para sordos).</p>
<ol>
  <li>En <b>Opciones → Subtítulos</b>, indique la clave de API de OpenSubtitles (y, para descargar más, el usuario y la contraseña de la cuenta) y/o la clave de SubDL. Las dos son gratuitas.</li>
  <li>En <b>Idiomas</b>, escriba los idiomas en el orden que quiera, separados por comas: <code>es, en</code>. Vale el código de dos letras con o sin país (<code>es</code>, <code>es-MX</code>, <code>pt-BR</code>) o el de tres (<code>spa</code>, <code>eng</code>).</li>
  <li>Después de <b>Buscar en TMDB</b>, haga clic en <b>Buscar subtítulos</b>. Buscar no gasta la cuota. La ventana de revisión muestra, para cada vídeo e idioma, el subtítulo elegido y los demás; cámbielo si quiere, o elija <b>No descargar</b>.</li>
  <li>Haga clic en <b>Renombrar</b>. Los vídeos que ya tienen el nombre correcto también reciben los subtítulos (el botón los cuenta).</li>
</ol>
<ul>
  <li><b>Cuál va primero</b>: el marcado con <b>★</b>, hecho para este mismo archivo (OpenSubtitles reconoce el vídeo por una "huella" tomada de su principio y su final, sin enviar el archivo); después los del mismo lanzamiento que indica el nombre del archivo (grupo, WEB o Blu-ray, resolución, códec); una versión extendida o sin cortes de un solo lado queda atrás. Por último, los más descargados. Los para sordos quedan un poco detrás de los demás.</li>
  <li><b>Lo que ya tiene</b>: un idioma que el vídeo ya tiene se omite: un subtítulo al lado con su nombre (<code>Película.es.srt</code>, <code>Película.spa.srt</code>) o una pista de subtítulos dentro del MKV (las "forced" no cuentan). Un subtítulo solo con <code>pt</code> cuenta para <code>pt-BR</code> y <code>pt-PT</code>.</li>
  <li><b>Cuota</b>: cuando la cuota de OpenSubtitles se agota a mitad de camino, los que faltan se buscan entre los de SubDL del mismo idioma; los que no se puedan descargar aparecen en un aviso. La ventana de revisión muestra cuántas descargas quedan hoy (con cuenta).</li>
  <li>Los subtítulos se guardan en UTF-8 (los que vienen en Windows-1252, comunes en español, se convierten). Un subtítulo con el mismo nombre que ya exista nunca se reemplaza. <b>Deshacer</b> el renombrado borra los subtítulos descargados.</li>
</ul>
<p><b>Limitaciones:</b> OpenSubtitles limita las descargas por día: 5 sin cuenta, desde 10 con una cuenta gratuita (más con VIP); el límite de SubDL es suyo. Sin la ★, el subtítulo puede ir adelantado o atrasado respecto a su archivo, sobre todo si es de otro lanzamiento; la calidad de la traducción depende de quien lo subió. Los subtítulos en más de un archivo (CD1, CD2) y los traducidos por máquina quedan fuera. Solo se ven las pistas de subtítulos de archivos MKV, no las de MP4. Solo se envían el número de la película en TMDB, los idiomas y la huella del vídeo (y, a OpenSubtitles, el usuario y la contraseña), nunca el nombre del archivo.</p>
<h4>Películas que faltan de la colección</h4>
<p>Con <b>Mostrar las películas que faltan de la colección</b> (activada por defecto), cuando una película elegida es parte de una colección en TMDB (una trilogía, una franquicia), la lista muestra filas atenuadas para las otras películas ya estrenadas de la colección que no están en ella, como <code>1999 · Título</code>, con la fecha de estreno, en el orden en que salieron. Son solo un aviso, como los episodios que faltan: no entran en conteos ni en el renombrado. <b>Haga clic en el total</b>, en la barra de abajo, para guardar la lista en un archivo CSV (colección, título y estreno). Para saber la colección, el programa hace una consulta más a TMDB por película y una por colección.</p>
<p>Quien decide qué entra en una colección es TMDB: a veces quedan fuera películas derivadas, y a veces entran películas que usted no consideraría parte de ella.</p>
<h4>Limitaciones</h4>
<ul>
  <li>La película se busca por el título y el año del nombre del archivo. Los nombres muy abreviados, solo con siglas o con el título en otro idioma pueden no encontrarse; en ese caso, corrija el título en la revisión.</li>
  <li>Las películas con el mismo nombre (remakes, por ejemplo) solo se distinguen por el año; sin el año en el nombre, la elección suele ir a la revisión.</li>
  <li>Las películas raras, regionales o muy recientes pueden no estar en TMDB, o estar sin traducción, sinopsis o póster.</li>
</ul>
<p class="tip">Solo se envían a TMDB el título y el año buscados (y, para las colecciones, el número de la película en TMDB), nunca el nombre completo del archivo ni las carpetas.</p>`,
  },
  {
    id: "r-series",
    title: "Serie (TMDB, AniList)",
    sub: true,
    html: `
<p>Da a cada episodio el nombre oficial de <b>TMDB</b>, con temporada, número y título, en el formato que Jellyfin reconoce. Usa la misma clave de la API que la regla de películas.</p>
${ex("Nombre.de.la.Serie.S01E03.720p.WEB-DL.mkv", "Nombre de la Serie (2008) - S01E03 - Título del Episodio.mkv")}
<div class="notice">
<p><b>Antes de usarla: el nombre de los archivos importa</b></p>
<p>Cada archivo se empareja con su episodio por lo que dice su nombre. El nombre de la serie puede venir del campo <b>Serie</b>, pero el episodio no: el nombre necesita <b>el número de temporada y de episodio</b> (<code>S01E03</code>, <code>1x03</code>; la temporada también puede venir de la carpeta) <b>o el título del episodio</b>. Un nombre sin ninguno de los dos (<code>Nombre de la Serie - parte A.mkv</code>, <code>video_0042.mkv</code>) no se encuentra y va a la revisión, donde se elige a mano.</p>
<p>Si sus archivos están lejos de eso, ponga encima de esta regla, en la lista, las reglas que arreglan el nombre (<b>Reemplazar texto</b>, <b>Reorganizar partes</b>, <b>Expresión regular</b>, <b>Numerar</b>): la búsqueda lee el nombre tal como sale de ellas.</p>
</div>
<ol>
  <li>En el campo <b>Serie</b>, escriba el nombre o el ID de TMDB, o pegue el enlace de la página de la serie (<code>themoviedb.org/tv/1396-…</code>). Así solo se buscan los archivos de esa serie; los que traen en el nombre otra serie (o el <code>[tmdbid-…]</code> de otra) quedan como están. <b>Vacío</b>, el programa separa los archivos por la serie que dice el nombre de cada uno (con el año, cuando los nombres lo traen; por el número, cuando traen <code>[tmdbid-…]</code>) y busca una serie cada vez, preguntando si hay dudas. Un archivo sin el nombre de la serie va con la serie de la mayoría de los archivos de la misma carpeta; en una carpeta sin nombres de serie, vale el nombre de la carpeta; en una carpeta con varias series y sin mayoría, queda fuera (el programa avisa cuántos).</li>
  <li>Haga clic en <b>Buscar episodios</b>. Si hay más de una serie parecida, elija la correcta. El programa descarga la lista completa de episodios y busca el de cada archivo visible.</li>
  <li>Los episodios encontrados con seguridad reciben <b>✓</b>. Los dudosos (<b>?</b>) y los no encontrados (<b>✕</b>) van a la ventana de revisión.</li>
</ol>
<h4>Cómo se encuentra cada episodio</h4>
<ul>
  <li><b>Por el número</b>, en varios formatos: <code>S01E02</code>, <code>s1e2</code>, <code>1x02</code>, <code>T1 E2</code>, <code>Temporada 1 Episodio 2</code>, <code>Ep 02</code> (la temporada viene de la carpeta, como <code>Season 2</code> o <code>Temporada 2</code>), numeración absoluta (<code>Nombre de la Serie - 125</code>), <code>201</code> (temporada 2, episodio 1) y fecha de emisión (<code>2024.03.15</code>). Los archivos con dos episodios (<code>S01E01E02</code>, <code>S01E01-02</code>) quedan como <code>S01E01-E02</code>. Los nombres hechos solo de bloques entre corchetes (<code>[Grupo][Nombre de la Serie][S03E22][Título][1080p]</code>, también con <code>【】</code>) también se leen: el bloque con el número, la serie antes y el título después.</li>
  <li><b>Por el título</b>: el texto después del número se compara con los títulos de todos los episodios, en su idioma, en inglés y en el idioma original, sin tener en cuenta acentos, mayúsculas ni pequeños errores; los números romanos valen como números (<code>Parte II</code> = <code>Parte 2</code>). Con todas las temporadas en una sola carpeta, cada una empezando por el 1 (<code>02 - Título.mkv</code>), el título elige la temporada. Sin número, basta el título si está claro.</li>
  <li><b>Cuando el número y el título no coinciden</b> (el nombre dice S02E08, pero el título es el de S01E03), el archivo queda con <b>?</b>, y la revisión muestra las dos opciones, sugiriendo la del título.</li>
  <li><b>Archivos sin el nombre de la serie</b> (<code>06 - Título.mkv</code>) fuera de la carpeta de la serie solo reciben el episodio del número si el título también coincide (o si no hay título); si no, van a la revisión.</li>
</ul>
<h4>Revisión</h4>
<p>Muestra lo que se leyó en el nombre, las sugerencias (clic, 1–9, o <b>Enter</b> para la primera) y todos los episodios de la serie por temporada, con un filtro por título o número (<code>s01e03</code>). <b>Ctrl+clic</b> marca más de un episodio de la misma temporada, para un archivo con varios episodios. Los subtítulos leídos igual reciben la misma elección. Solo se buscan vídeos y subtítulos; las imágenes, los <code>.nfo</code> y otros archivos de la lista quedan como están.</p>
<ul>
  <li><b>Plantilla</b>: <code>{serie}</code>, <code>{original}</code> (nombre original), <code>{año}</code> (estreno), <code>{temporada}</code>, <code>{episodio}</code> (dos dígitos), <code>{título}</code> e <code>{id}</code>. La predeterminada es <code>{serie} ({año}) - S{temporada}E{episodio} - {título}</code>.</li>
  <li>También, leídos desde dentro del vídeo: <code>{resolución}</code> y <code>{códec de vídeo}</code>, como en la regla Película (TMDB). No entran en el nombre de las carpetas de la serie.</li>
  <li><b>Más variables</b>, de TMDB: <code>{cadena}</code>, <code>{género}</code>, <code>{géneros}</code>, <code>{idioma}</code>, <code>{país}</code>, la fecha de emisión del episodio y <code>{absoluto}</code>, el número del episodio contado desde el primero, sin los especiales (útil para anime: <code>{serie} - {absoluto:3}</code>). Fechas: <code>{fecha}</code> (<code>2010-05-03</code>), <code>{mes}</code> (<code>05</code>), <code>{día}</code> y el nombre del mes en el idioma de la variable: <code>{nombre del mes}</code> da <code>mayo</code>, <code>{month name}</code> da <code>May</code> y <code>{nome do mês}</code> da <code>maio</code>. Del nombre del archivo: <code>{edición}</code>, <code>{fuente}</code> y <code>{grupo}</code> (también <code>[Grupo]</code> al principio, común en anime).</li>
  <li><b>Lo que ya está al lado</b>: un <code>tvshow.nfo</code> en la carpeta de los episodios (o en la de arriba, cuando están en una carpeta de temporada) dice qué serie es por su ID de TMDB o IMDb, sin buscar por el nombre. Si escribe la serie en la regla, vale lo que escribió.</li>
  <li><b>Partes opcionales</b>: una parte entre <code>&lt; &gt;</code> solo aparece cuando todas las variables dentro de ella tienen valor (los signos <code>&lt; &gt;</code> no van al nombre). <code>{serie} - S{temporada}E{episodio}&lt; - {título}&gt;</code> solo pone el guion cuando el episodio tiene título. También vale en la plantilla de la carpeta de la serie.</li>
  <li><b>Alternativa</b>: <code>{a|b}</code> pone la primera de esas variables que tenga valor. <code>{título|año}</code> da el título, o el año cuando no hay título. Puede tener más de dos (<code>{a|b|c}</code>) y funciona dentro de una parte opcional.</li>
  <li><b>Cifras</b>: <code>{variable:N}</code> escribe los números con N cifras. <code>{a:3}</code> da <code>005</code> en vez de <code>05</code>, y <code>{a:1}</code> da <code>5</code>, sin ceros delante. Los decimales (<code>12.1</code>) y el disco antes de la pista (<code>1-05</code>) quedan como están; el texto sin número no cambia.</li>
  <li>Los episodios sin traducción reciben el título en inglés (o en el idioma original), en lugar de "Episodio 4".</li>
  <li>Los subtítulos conservan el idioma al final del nombre: <code>….pt-BR.srt</code>.</li>
  <li>Los archivos que no están en la serie elegida quedan como están. Cada archivo guarda su serie; buscar de nuevo no toca los ya resueltos.</li>
</ul>
<h4>Carpetas de la serie y la temporada</h4>
<p>Con <b>Organizar en carpetas de la serie y la temporada</b>, cada episodio va a la estructura que recomienda Jellyfin: <code>Series/Nombre de la Serie (2008) [tmdbid-12345]/Season 01/…</code>. La carpeta de la serie se crea junto a la carpeta donde ya está la serie (si los archivos están en <code>Nombre de la Serie/Season 1</code>, la nueva carpeta queda junto a <code>Nombre de la Serie</code>, no dentro); con archivos sueltos, dentro de su carpeta. Una carpeta cuenta como de la serie cuando tiene su nombre o cuando todos los vídeos que contiene son episodios de esa serie, aunque se llame de otro modo (<code>Nombre de la Serie Completa 1080p</code>); la carpeta personal, Descargas, Vídeos y la raíz de un disco nunca cuentan. El nombre de la carpeta de la serie tiene su propia plantilla, con las mismas variables (predeterminada <code>{serie} ({año}) [tmdbid-{id}]</code>; use <code>{original}</code> para el nombre original). Las carpetas antiguas que quedan vacías se borran. Si la carpeta antigua de la serie aún tiene otros archivos (extras, imágenes, un archivo en conflicto), se queda junto a la nueva y pasa a llamarse <code>Nombre de la Serie (2008) [tmdbid-12345] (archivos adicionales)</code>, para que ambas aparezcan juntas; antes de renombrar, la barra inferior muestra <b>carpeta(s) con sobrantes</b>, y la confirmación dice qué carpeta y cuántos archivos se quedan. Después de renombrar, la carpeta antigua puede desaparecer (<code>Dibujos/Nombre de la Serie</code>) y los archivos aparecen en la nueva, junto a ella (<code>Dibujos/Nombre de la Serie (2008) [tmdbid-12345]/Season 01/…</code>): <b>no se borró nada, solo se movió</b>. <b>Deshacer</b> lo devuelve todo a su sitio y vuelve a crear las carpetas antiguas.</p>
<h4>Metadatos (.nfo e imágenes)</h4>
<p>Con <b>Guardar .nfo e imágenes para Jellyfin</b>, al renombrar el programa crea:</p>
<ul>
  <li>en la <b>carpeta de la serie</b>: <code>tvshow.nfo</code> (título, título original, año, sinopsis, estado, duración, nota, clasificación por edad, géneros, palabras clave, cadenas y estudios, creadores, tráiler, reparto con personajes y los ID de TMDB, IMDb y TVDB), <code>poster.jpg</code> y el póster de cada temporada (<code>season01-poster.jpg</code>); opcionalmente <code>fanart.jpg</code> y <code>logo.png</code>;</li>
  <li>en la <b>carpeta de la temporada</b>: <code>season.nfo</code>, con el nombre y la sinopsis de la temporada;</li>
  <li>junto a <b>cada episodio</b>: <code>….nfo</code> (título, temporada y número, fecha de emisión, sinopsis, duración, nota, dirección, guion y actores invitados) y la miniatura <code>…-thumb.jpg</code>.</li>
</ul>
<p>Los archivos de la serie y la temporada solo se crean cuando los episodios están en una carpeta de la serie (con su nombre o con <code>[tmdbid-…]</code>), la de antes o la que crea la opción de carpetas. Los episodios que ya tienen el nombre correcto también reciben los metadatos. Los archivos que ya existen no se reemplazan, y deshacer borra lo creado.</p>
<h4>Subtítulos</h4>
<p><b>Descargar subtítulos</b> funciona como en la regla de películas (vea <b>Película (TMDB)</b>), para los episodios ya identificados: la búsqueda es por serie, temporada y episodio, y el subtítulo recibe el nombre nuevo del episodio (<code>Serie (2008) - S01E03 - Título.es.srt</code>). Limitaciones: solo con TMDB (con AniList, la opción desaparece); un archivo con varios episodios se queda sin subtítulo; un paquete de temporada de SubDL se abre y solo se usa el subtítulo del episodio, cuando su nombre dice cuál es (<code>S01E03</code>).</p>
<h4>Episodios que faltan</h4>
<p>Con <b>Mostrar los episodios que faltan</b> (activada por defecto), tras la búsqueda la lista muestra filas atenuadas para los episodios ya emitidos que ningún vídeo de la lista tiene, por ejemplo <code>S01E16 · Título</code>, con la fecha de emisión. Quedan en el lugar del episodio, entre sus vecinos, y la barra inferior dice cuántos faltan. Son solo un aviso: no se pueden seleccionar y no cuentan para totales, renombrado ni la exportación de la lista. <b>Haga clic en ese total</b> para guardar los episodios faltantes en un archivo CSV (serie, temporada, episodio, código, título y fecha de emisión), para abrir en una hoja de cálculo. La ventana se abre en la carpeta de la serie, con su nombre en el del archivo; se puede elegir otro lugar.</p>
<p>Solo se revisan las temporadas de las que la lista tiene al menos un vídeo; las temporadas enteras que no tienes no aparecen. Tampoco los episodios aún no emitidos o sin fecha. Un episodio que solo tiene el subtítulo cuenta como faltante.</p>
<h4>Anime en AniList</h4>
<p>En <b>Dónde buscar</b>, <b>AniList</b> busca el anime en AniList (anilist.co), sin cuenta ni clave; no se usan TMDB ni su clave. El campo <b>Serie</b> acepta también el id de AniList o el enlace de su página (<code>anilist.co/anime/…</code>). En AniList cada temporada es una entrada aparte, enlazada con la siguiente: el programa parte de la primera temporada y junta las que siguen, en orden (temporada 1, 2…), y la búsqueda no ofrece las temporadas siguientes como series aparte. Las películas y los especiales quedan fuera. La numeración corrida (<code>Serie - 30</code>) se cuenta a través de las temporadas, como con TMDB.</p>
<ul>
  <li>Los <b>títulos de los episodios</b> vienen de la lista de los servicios de streaming que guarda AniList (<code>Episode 3 - Título</code>), en inglés; cuando no existe, el episodio queda sin título, y la búsqueda por el título en el nombre del archivo no ayuda.</li>
  <li>La <b>fecha de emisión</b> viene de la programación de AniList (solo las últimas 50 de cada temporada); un episodio que falta en ella queda con la fecha del anterior (varios se estrenan el mismo día), y el primero, con la fecha de estreno.</li>
  <li>La serie queda con el nombre del archivo cuando es uno de sus nombres en AniList (una traducción, por ejemplo); si no, con el nombre en inglés, o el principal, en japonés romanizado.</li>
  <li><code>{id}</code> es el de AniList, y el <code>[tmdbid-{id}]</code> de la plantilla pasa a <code>[anilistid-…]</code>, que entiende el plugin de AniList para Jellyfin. Más variables: <code>{género}</code>, <code>{géneros}</code> y <code>{país}</code>.</li>
  <li>Los archivos <b>.nfo e imágenes</b> son solo de TMDB: con AniList, la opción desaparece.</li>
</ul>
<h4>Limitaciones</h4>
<ul>
  <li><b>AniList</b>: las temporadas siguen sus entradas enlazadas como continuación; una segunda parte registrada como otra cosa (un especial, un ONA aparte) queda fuera, y sus archivos van a la revisión. Solo uso no comercial y unas 30 consultas por minuto (el programa espera 2 segundos entre ellas; una serie con 6 temporadas tarda unos 12 segundos la primera vez). Las respuestas se guardan 7 días como mucho.</li>
  <li>Una serie solo se reconoce en el nombre del archivo por los nombres que tiene en TMDB (en cualquier idioma o país) o por lo que usted escribió. Un apodo o una sigla que TMDB no conoce hace que el archivo cuente como de otra serie: escriba la serie, o use ese nombre en el campo.</li>
  <li>La división en temporadas es la de TMDB. El anime y las series con numeración corrida a veces están divididos de otra forma allí, y los números del archivo no coinciden; revíselo en la revisión.</li>
  <li>Los especiales (temporada 0) suelen tener una numeración distinta de la de su emisión y casi siempre necesitan revisión.</li>
  <li>Un archivo con varios episodios solo se reconoce cuando son de la misma temporada.</li>
  <li>Los episodios que faltan siguen la numeración de TMDB: si tus archivos usan otra (la del DVD, o la numeración corrida de los animes), la lista de lo que falta puede salir mal. Los episodios ocultos por el filtro también cuentan como faltantes.</li>
</ul>
<p class="tip">Solo se envía a TMDB (o a AniList) el nombre de la serie, nunca los nombres de los archivos ni las carpetas.</p>`,
  },
  {
    id: "r-music",
    title: "Música (etiquetas)",
    sub: true,
    html: `
<p>Da a cada canción el nombre que viene de las <b>etiquetas</b> guardadas en el archivo (ID3 del MP3, Vorbis del FLAC y del OGG, MP4 del M4A y otras), sin necesidad de internet.</p>
${ex("01.mp3", "01 - Título de la Canción.mp3")}
<ul>
  <li><b>Plantilla</b>: <code>{pista}</code>, <code>{título}</code>, <code>{artista}</code>, <code>{artista del álbum}</code>, <code>{álbum}</code>, <code>{año}</code>, <code>{disco}</code>, <code>{género}</code> y <code>{compositor}</code>. La predeterminada es <code>{pista} - {título}</code>. La pista tiene dos dígitos (tres en álbumes con más de 99 pistas). <code>{disco}</code> solo tiene valor en álbumes de más de un disco: en un álbum de un solo disco, queda vacío.</li>
  <li>También: <code>{total de pistas}</code>, <code>{total de discos}</code>, <code>{año original}</code> (el del primer lanzamiento del álbum; sin él, el <code>{año}</code>), <code>{sello}</code>, <code>{catálogo}</code> (el número de catálogo), <code>{artista invertido}</code> y <code>{artista del álbum invertido}</code> (cómo se ordena el artista: <i>Beatles, The</i>; sin esa etiqueta, el propio nombre) e <code>{inicial}</code> (la primera letra del artista del álbum invertido, para carpetas como <code>{inicial}/{artista del álbum invertido}</code>). Estos vienen de las etiquetas o, cuando el álbum se encuentra en MusicBrainz, del lanzamiento. Del propio archivo: <code>{formato}</code> (<code>FLAC</code>, <code>MP3</code>…), <code>{tasa de bits}</code> (en kbit/s), <code>{muestreo}</code> (en kHz: <code>44.1</code>, <code>96</code>) y <code>{bits}</code> (<code>16</code>, <code>24</code>; los formatos con pérdida, como MP3, no tienen). Ejemplo: <code>{álbum} &lt;[{formato} {bits}-{muestreo}]&gt;</code> da <code>Álbum [FLAC 24-96]</code>.</li>
  <li><b>Fecha de lanzamiento</b>, de las etiquetas (o de MusicBrainz, al identificar el álbum). Fechas: <code>{fecha}</code> (<code>2010-05-03</code>), <code>{mes}</code> (<code>05</code>), <code>{día}</code> y el nombre del mes en el idioma de la variable: <code>{nombre del mes}</code> da <code>mayo</code>, <code>{month name}</code> da <code>May</code> y <code>{nome do mês}</code> da <code>maio</code>. Limitación: muchos archivos solo tienen el año; entonces mes y día quedan vacíos.</li>
  <li><b>Partes opcionales</b>: una parte entre <code>&lt; &gt;</code> solo aparece cuando todas las variables dentro de ella tienen valor (los signos <code>&lt; &gt;</code> no van al nombre). En la plantilla de carpetas, <code>{artista del álbum}/{álbum}/&lt;CD{disco}&gt;</code> crea la carpeta <code>CD1</code>, <code>CD2</code>… solo en álbumes de más de un disco.</li>
  <li><b>Alternativa</b>: <code>{a|b}</code> pone la primera de esas variables que tenga valor. <code>{título|año}</code> da el título, o el año cuando no hay título. Puede tener más de dos (<code>{a|b|c}</code>) y funciona dentro de una parte opcional.</li>
  <li><b>Cifras</b>: <code>{variable:N}</code> escribe los números con N cifras. <code>{a:3}</code> da <code>005</code> en vez de <code>05</code>, y <code>{a:1}</code> da <code>5</code>, sin ceros delante. Los decimales (<code>12.1</code>) y el disco antes de la pista (<code>1-05</code>) quedan como están; el texto sin número no cambia.</li>
  <li>Sin número de pista en las etiquetas, queda el número con el que empieza el nombre del archivo (<code>07. Título.flac</code>), para no perder el orden. Un álbum queda con un solo año, el más común entre sus pistas: una pista extra de una reedición no recibe una carpeta aparte.</li>
  <li>Las etiquetas vacías no dejan restos: con <code>{pista} - {artista} - {título}</code>, una canción sin artista queda <code>01 - Título</code>.</li>
  <li><b>Número de disco en la pista</b>: en álbumes con más de un disco, la pista queda <code>1-01</code>, <code>2-01</code>…, para que los discos no se mezclen.</li>
  <li><b>El álbum como un todo</b>: las pistas de un mismo álbum (misma carpeta, o discos <code>CD 01</code>/<code>CD 02</code>, y mismo nombre de álbum) comparten los datos del álbum. Una pista sin año recibe el de las demás. Sin "artista del álbum" en las etiquetas, vale el artista de la mayoría de las pistas (sin contar colaboraciones como <code>feat.</code>); sin mayoría, el álbum es una recopilación: <code>Various Artists</code>, como lo reconoce Jellyfin. Así, las pistas de un álbum nunca se reparten en carpetas distintas.</li>
  <li><b>Pistas extra</b>: en una carpeta donde la mayoría de las canciones son de un álbum, las pistas con otro álbum en las etiquetas (un sencillo, una banda sonora) y numeradas después de las del álbum (<code>15</code>, <code>16</code> después de <code>01</code>–<code>14</code>) quedan con ese álbum.</li>
  <li><b>Sencillos</b>: una canción con el álbum <code>Single</code> en las etiquetas toma su propio título como álbum.</li>
  <li><b>Recopilaciones</b>: en un álbum donde la mayoría de las pistas son de otros artistas que el del álbum, el artista entra en el nombre antes del título cuando la plantilla no lo tiene: <code>03 - Artista de la Pista - Título</code>.</li>
  <li>Si el nombre del álbum ya trae un año (<code>Programa 12 (2025-05-04)</code>), se omite el <code>({año})</code> de la plantilla.</li>
  <li>Las canciones <b>sin título en las etiquetas</b> quedan como están y reciben <b>✕</b> (salvo que la búsqueda las identifique por el sonido, vea abajo); la tarjeta muestra cuántas son. Con <b>Completar con el nombre del archivo</b>, el programa lee <code>01 - Artista - Título</code>, <code>01. Título</code> o <code>1-05 Título</code> del nombre, y el álbum y el año de la carpeta (<code>Artista - Álbum (1975)</code>, o solo <code>Álbum</code>; una carpeta <code>CD2</code> da el disco). Las etiquetas existentes tienen prioridad.</li>
</ul>
<h4>Carpetas de artista y álbum</h4>
<p>Con <b>Organizar en carpetas de artista y álbum</b>, cada canción va a <code>{artista del álbum}/{álbum} ({año})</code> (la plantilla de carpetas se puede cambiar; cada <code>/</code> es un nivel). Así, <code>Música/Artista/Album/CD1/01.mp3</code> queda <code>Música/Artista/Álbum (1975)/01 - Título de la Canción.mp3</code>: las carpetas que ya llevan el nombre del artista, del álbum o de un disco (o que están dentro de la del artista) se reemplazan, no reciben carpetas nuevas dentro. Los archivos sueltos reciben las carpetas dentro de la carpeta donde están, salvo que solo tenga canciones de un álbum: entonces cuenta como la carpeta del álbum, tenga el nombre que tenga (<code>Música/Carpeta nueva/01.flac</code> queda <code>Música/Artista/Álbum (1975)/01 - Título de la Canción.flac</code>), y se borra si queda vacía. La carpeta personal, Música, Descargas y las demás carpetas del sistema nunca cuentan. Las pequeñas diferencias de escritura entre la carpeta y la etiqueta no molestan. Una carpeta que ya reúne álbumes con su nombre (<code>Programa (1000 - ..)</code> para los álbumes "Programa 1037 - …") se trata como colección: los álbumes quedan directamente en ella, sin carpetas de artista en medio.</p>
<div class="notice">
<p><b>Las carpetas de canciones sueltas quedan juntas</b></p>
<p>Una carpeta en la que casi todas las canciones son de álbumes distintos, con una o dos pistas de cada uno (como una carpeta <code>Sueltas</code>, <code>Misc</code> o <code>Singles</code>), se trata como una recopilación que usted armó. Sus canciones se renombran como siempre, pero <b>quedan en ella</b>: no van cada una a la carpeta de su álbum. El álbum de cada una sigue en sus etiquetas.</p>
<p>La regla exacta: al menos 3 canciones, de al menos 3 álbumes, y al menos 4 de cada 5 de esos álbumes con una o dos pistas en la carpeta. La carpeta de un álbum de verdad, con varias pistas del mismo disco, se organiza como siempre. Si quiere separar las sueltas de todos modos, muévalas a mano a las carpetas que prefiera.</p>
</div>
<p><b>Copias</b>: si dos copias idénticas de un álbum (encontradas por el programa, como en <b>Copias idénticas</b>) irían al mismo lugar, la primera sigue y la otra se queda donde está, marcada como copia.</p>
<ul>
  <li>Las letras (<code>.lrc</code>) con el nombre de una canción reciben su nuevo nombre y van con ella.</li>
  <li>Las portadas, <code>.cue</code>, <code>.log</code> y otros archivos de la carpeta del álbum van con la mayoría de las canciones de la carpeta (las canciones de otros álbumes que no son pistas extra siguen el suyo). Las subcarpetas sin canciones (<code>Artwork</code>, <code>Scans</code>) van con él, con el mismo nombre; también los archivos de la carpeta de encima de <code>CD 01</code>/<code>CD 02</code>. Si dos de esos archivos quedarían con el mismo nombre (un <code>log.txt</code> en cada disco), reciben el nombre de su carpeta: <code>log (CD 01).txt</code>.</li>
  <li>Los nombres de artista o álbum que solo difieren en mayúsculas (<code>nombre del artista</code> y <code>Nombre del Artista</code>, de etiquetas de álbumes distintos) quedan en una sola carpeta: la que ya existe en el disco o la grafía más usada.</li>
  <li>Las carpetas antiguas que quedan vacías se borran; deshacer lo devuelve todo a su sitio.</li>
  <li><b>Crear una lista de reproducción por álbum</b>: después de renombrar, graba <code>&lt;nombre de la carpeta&gt;.m3u8</code> en la carpeta de cada álbum, con las canciones (de todos los discos) en el orden de los discos y pistas. También vale para los álbumes que ya tienen los nombres correctos (cuentan en el botón <b>Renombrar</b>). No se crea si el álbum ya tiene una lista (<code>.m3u</code> o <code>.m3u8</code>): esa se actualiza con los nombres nuevos, y el resumen del final lo avisa. Un <code>.cue</code> no lo impide, porque describe el disco y la mayoría de los reproductores no lo usan como lista. Tampoco se crea para una canción sola. Deshacer borra la lista creada.</li>
  <li>Los archivos <code>.cue</code> y las listas <code>.m3u</code> pasan a citar los nuevos nombres de las canciones (vea <b>Renombrar y deshacer</b>).</li>
</ul>
<h4>Corregir con MusicBrainz</h4>
<p>Para etiquetas erróneas o incompletas, <b>Buscar en MusicBrainz</b> identifica cada álbum en <b>MusicBrainz</b> (musicbrainz.org), una base de datos abierta de música. No necesita clave.</p>
<ul>
  <li>Las canciones visibles se agrupan por álbum. Si las etiquetas ya traen el ID de MusicBrainz, el álbum se reconoce directamente (cuando todas las canciones están en él; si no, pasa a la revisión); si no, el programa busca por artista y nombre del álbum.</li>
  <li>Un mismo álbum suele tener varias ediciones (original, remasterizada, de lujo, de otro país). El programa comprueba el <b>número de pistas</b>, el <b>título</b> y la <b>duración de cada una</b> (una pista con el mismo número pero otro título y otra duración no cuenta): si la mejor edición encontrada coincide con las canciones, se elige sola (<b>✓</b>). Si no, se abre la revisión, con portada, fecha, país, formato, sello y número de pistas (en verde cuando es igual al de la carpeta), para que usted elija, o <b>Ninguno</b> para mantener las etiquetas.</li>
  <li>Con el álbum elegido, los nombres y las carpetas usan los datos de MusicBrainz: título de cada pista, artista, artista del álbum, nombre del álbum y año, con la grafía oficial. Cada canción se empareja con su pista por disco y número (o por título).</li>
  <li><b>Guardar la portada del álbum</b>: graba <code>cover.jpg</code> (del Cover Art Archive) en la carpeta del álbum, si aún no tiene portada. Si la edición elegida no tiene imagen (un casete, por ejemplo), se usa la portada del álbum. Deshacer borra la portada creada.</li>
  <li><b>Grabar las etiquetas en los archivos</b>: después de renombrar, graba en las canciones identificadas todo lo que MusicBrainz sabe de ellas: título, artistas (también cada uno por separado), artista del álbum, álbum, fecha y fecha original, pista y disco (con los totales), sello, número de catálogo, código de barras, país, tipo de lanzamiento, soporte, ISRC, géneros, idioma, compositor, letrista, productor, arreglista, mezcla y los IDs de MusicBrainz, que usan Jellyfin, Navidrome y Plex. Para eso, cada álbum se consulta una vez más (unos 2 segundos). También vale para las canciones que ya tienen el nombre correcto (cuentan en el botón <b>Renombrar</b>). Una canción que ya tiene todo esto no se vuelve a grabar. Funciona con MP3 (grabado en ID3v2.3), FLAC, OGG, Opus y M4A.</li>
  <li><b>Borrar las demás etiquetas</b>: quita lo que no viene de MusicBrainz (comentarios, direcciones de sitios, datos de quien ripeó, "codificado por"), y en MP3 también las etiquetas secundarias (ID3v1 y APE). Quedan las letras, el ReplayGain, el BPM, la hoja de pistas incrustada (CUESHEET) y el género y el compositor cuando MusicBrainz no los tiene. Desmarcada, las demás etiquetas quedan como están.</li>
  <li><b>Incluir la portada en los archivos</b>: quita las imágenes que ya están dentro de cada canción y pone la portada del álbum (del Cover Art Archive o, si no hay, la <code>cover.jpg</code> de la carpeta). En <b>Tamaño de la portada</b>, elija 500 o 250 px para radios de coche y reproductores antiguos que se traban con portadas grandes o no las muestran; las canciones también quedan más pequeñas. La <code>cover.jpg</code> guardada en la carpeta sigue grande. Limitación: una portada sacada de la carpeta (cuando el Cover Art Archive no tiene) entra con el tamaño que tenga, sin reducirse.</li>
  <li>La grabación se hace en una copia, que solo ocupa el lugar del archivo al final; un corte a mitad no estropea la canción. Si se cierra el programa o se apaga el equipo durante la grabación, la próxima vez que Renome abra las canciones ya grabadas van al <b>Historial</b>, para deshacer, y la copia que se estaba grabando se borra. Deshacer devuelve las etiquetas y las imágenes antiguas (en MP3 y M4A, los campos raros que quitó la limpieza y que no son texto, como datos binarios de otros programas, y las etiquetas ID3v1 y APE no vuelven). La fecha de modificación cambia, para que los reproductores relean las etiquetas. Como el contenido cambia, un torrent ya no reconoce esos archivos.</li>
  <li>Las consultas se espacian (una cada 2 segundos), como pide MusicBrainz; cada álbum tarda unos 4 segundos.</li>
</ul>
<h4>Discogs (opcional)</h4>
<p>Muchos lanzamientos regionales, independientes y en vinilo están en <b>Discogs</b> (discogs.com) y no en MusicBrainz. Para usarlo, cree una cuenta gratuita en Discogs, genere un <b>token personal</b> en Configuración → Desarrolladores y péguelo en <b>Opciones → Discogs</b>.</p>
<ul>
  <li>Con el token, cuando MusicBrainz no encuentra nada para un álbum, el programa busca en Discogs por el artista y el nombre del álbum. La edición se comprueba igual (número de pistas, títulos y duraciones): si coincide, se elige sola; si no, va a la revisión. Los candidatos de Discogs llevan la etiqueta <b>Discogs</b>.</li>
  <li>En la revisión, junto a <b>Buscar</b>, elija <b>MusicBrainz</b> o <b>Discogs</b> para buscar en la otra fuente, aunque la primera haya encontrado algo.</li>
  <li>Los nombres, las carpetas, la portada (<code>cover.jpg</code>), las pistas que faltan y la escritura de etiquetas funcionan igual, con los datos de Discogs. Los discos salen de la posición de las pistas (<code>1-05</code>, <code>CD2-3</code>); las caras de vinilo (<code>A1</code>, <code>B2</code>) se numeran como un solo disco. Las partes de una obra (<code>Sonata: I. Allegro</code>) cuentan como pistas.</li>
  <li>A Discogs van solo el artista y el nombre del álbum, con su token. El token se guarda solo en este equipo, en el almacén de contraseñas del sistema.</li>
</ul>
<p><b>Limitaciones de Discogs:</b> solo se consulta cuando MusicBrainz no encuentra nada (o cuando usted lo elige en la revisión); la identificación por el sonido no usa Discogs. Las etiquetas escritas desde Discogs tienen menos campos: título, artistas, álbum, fechas, pista, disco, sello, número de catálogo, código de barras y géneros (los estilos de Discogs primero); no hay IDs, compositores, productores ni ISRC, y por eso una búsqueda posterior vuelve a buscar el álbum. La portada es la de Discogs, en general de unos 600 píxeles: el <b>Tamaño de la portada</b> puede reducirla (500 o 250), pero la opción Grande se queda con los 600. El país y el año que muestra la revisión son los de la búsqueda; el número de pistas solo aparece después de abrir la edición. Discogs acepta unas 60 consultas por minuto; cada álbum usa 2 o 3.</p>
<h4>Identificar por el sonido (AcoustID)</h4>
<p>Con <b>Identificar por el sonido</b> marcado (el valor por defecto), la búsqueda también se ocupa de las canciones <b>sin título en las etiquetas</b> (incluso con <b>Completar con el nombre del archivo</b>, ya que el nombre es solo una suposición) y de los álbumes que la búsqueda por nombre no encontró o dejó en duda. En un álbum en duda, lo escuchado solo vale si identifica un lanzamiento con seguridad; si no, quedan los candidatos de la búsqueda. El programa escucha los 2 primeros minutos de cada canción, calcula la <b>huella digital</b> del sonido y le pregunta a <b>AcoustID</b> (acoustid.org), una base abierta de huellas digitales ligada a MusicBrainz, qué grabación es.</p>
<ul>
  <li>Las canciones de una carpeta se reparten entre los álbumes en que aparecen sus grabaciones: si la mayoría está en un mismo álbum, la carpeta es ese álbum; las canciones sueltas van al álbum de estudio del artista, y no a un recopilatorio.</li>
  <li>Entre las ediciones, queda la de número de pistas más cercano al de la carpeta y, después, la más antigua. Si las pistas y las duraciones coinciden, el álbum se elige solo; si no, va a la revisión, que muestra <b>Por el sonido:</b> con el álbum escuchado.</li>
  <li>Una canción sin título recibe el nombre de su pista en el álbum elegido. Una canción que AcoustID no conoce aparece como <b>no reconocida</b> y queda como está.</li>
  <li>Lee MP3, FLAC, OGG, M4A (AAC y ALAC), WAV y AIFF. Otros formatos, como Opus y WMA, no se escuchan.</li>
  <li>Cada canción tarda cerca de medio segundo en escucharse y consultarse.</li>
</ul>
<p class="tip">A AcoustID solo van la huella digital del sonido y la duración de cada canción, nunca el archivo.</p>
<h4>Pistas que faltan</h4>
<p>Con <b>Mostrar las pistas que faltan</b> (activada por defecto), en los álbumes identificados en MusicBrainz la lista muestra filas atenuadas para las pistas de la edición elegida que ninguna canción de la lista tiene, como <code>05 · Título</code> (<code>2-05</code> en álbumes con más de un disco), en su lugar, entre sus vecinas. Son solo un aviso: no entran en conteos ni en el renombrado. <b>Haga clic en el total</b>, en la barra de abajo, para guardar la lista en un archivo CSV (álbum, disco, pista y título). No hay consulta extra: la lista de pistas es la que ya se usó para identificar el álbum.</p>
<p>La lista es la de la <b>edición elegida</b>: una edición con pistas extra muestra las extra que usted no tiene; otra edición del mismo álbum puede no tenerlas. En la revisión se puede elegir la edición que corresponde a la suya. Una canción que no casó con ninguna pista de la edición hace que su pista aparezca como faltante.</p>
<h4>Limitaciones</h4>
<ul>
  <li>Sin la búsqueda en MusicBrainz, el nombre depende solo de las etiquetas: etiquetas erróneas dan nombres erróneos.</li>
  <li>Los lanzamientos regionales, independientes, piratas o muy recientes pueden no estar en MusicBrainz; entonces valen las etiquetas, salvo que Discogs (con su token) los tenga.</li>
  <li>Por el sonido, grabaciones muy parecidas (una remasterización, una regrabación del propio artista) pueden confundirse. Cuando el álbum queda en duda, va a la revisión.</li>
  <li>Las canciones en Opus, WMA y otros formatos no citados arriba no se escuchan.</li>
  <li>En MP3 y M4A, deshacer la grabación de etiquetas no devuelve los campos raros que no son texto ni las etiquetas ID3v1 y APE.</li>
  <li>En MP3, varios valores de un campo (géneros, compositores…) quedan en uno solo (<code>Rock; Pop</code>), porque el formato usado para que Windows lea las etiquetas (ID3v2.3) guarda un valor por campo.</li>
</ul>
<p class="tip">Sin <b>Grabar las etiquetas en los archivos</b>, Renome solo lee las etiquetas; las canciones en sí no cambian. A MusicBrainz solo se envían el artista y el nombre del álbum.</p>`,
  },
  {
    id: "r-book",
    title: "Libro (Open Library, Google Books)",
    sub: true,
    html: `
<p>Da a cada libro el nombre que viene de <b>Open Library</b> (openlibrary.org), un catálogo abierto de libros mantenido por el Internet Archive. Cuando Open Library no conoce el ISBN, se busca en <b>BrasilAPI</b> (brasilapi.com.br), que reúne los registros de las editoriales brasileñas en la CBL. Ninguno de los dos necesita clave. Con una clave gratuita de <b>Google Books</b>, puesta en <b>Opciones → Google Books</b>, los libros que Open Library no resuelve se buscan también allí (vea abajo).</p>
${ex("titulo_del_libro_autor.pdf", "Nombre del Autor - Título del Libro (1937).pdf")}
<ol>
  <li>Añada la regla y haga clic en <b>Buscar en Open Library</b>. Para cada libro (EPUB, MOBI, AZW3, PDF, FB2 y DjVu), el programa lee los datos grabados en el propio archivo: título, autor, ISBN, editorial y serie. Lo que falte viene del nombre del archivo.</li>
  <li>Con el <b>ISBN</b>, el libro se encuentra directamente, en su edición exacta (<b>✓</b>), en Open Library o en BrasilAPI. Sin él, el programa busca por el título y el autor: si coinciden sin dudas, el libro se elige solo; si no (<b>?</b>), se abre la revisión, con portada, autores, título original y número de ediciones, para que usted elija (haga clic o pulse 1–9).</li>
  <li>La revisión muestra la <b>portada del archivo</b> en la esquina (la portada del EPUB, o la primera página del PDF) y la compara con la portada de cada candidato: los de portada con el mismo dibujo reciben la marca <b>misma portada</b>. Una edición con otra portada (otro arte, otra editorial) no la recibe, aunque sea el libro correcto.</li>
  <li>Si ninguno sirve, corrija el título o el autor en la ventana y busque de nuevo. También se puede buscar solo por el autor, con el título vacío, y ver sus libros. O marque <b>Ninguno de estos</b> (<b>✕</b>: el archivo queda como está).</li>
</ol>
<h4>Google Books</h4>
<p>Google Books es opcional y necesita una clave gratuita: cree un proyecto en Google Cloud, active la <b>Books API</b> y cree una clave de API. El orden importa: active primero la Books API (en <b>APIs y servicios → Biblioteca</b>, busque "Books API" y haga clic en <b>Habilitar</b>), porque el campo <b>Restricciones de API</b> de la clave solo muestra las APIs ya activadas en el proyecto. Después, en <b>Credenciales → Crear credenciales → Clave de API</b>, elija la Books API en las restricciones y deje <b>Restricciones de aplicaciones</b> en <b>Ninguna</b> (las otras opciones son para sitios web, direcciones IP y apps de móvil). No pide cuenta de servicio ni tarjeta. Con ella:</p>
<ul>
  <li>un ISBN que Open Library no conoce se busca primero en Google Books, después en BrasilAPI; la edición encontrada viene con editorial, año, páginas, idioma, portada y sinopsis;</li>
  <li>un libro sin ISBN que Open Library no elige solo se busca también por el título y el apellido del autor, primero en ediciones en el idioma de los títulos. Cuando Google encuentra el mismo libro que Open Library (título y autor coinciden), solo completa lo que falta: editorial, páginas, ISBN, portada y sinopsis; el resto viene de Open Library. Los otros libros que encuentra entran en la revisión después de los de Open Library, marcados <b>Google Books</b>. Si Open Library no encuentra nada, un libro de Google que coincide sin duda se elige solo;</li>
  <li>la portada guardada viene de Open Library; solo cuando no tiene, de Google Books, en el mayor tamaño que da (unos 800 píxeles de ancho). La sinopsis de Google va al <code>metadata.opf</code> cuando Open Library no tiene una.</li>
</ul>
<h4>De dónde vienen los datos</h4>
<ul>
  <li><b>EPUB</b>: título, autores, ISBN, editorial, idioma y serie (de los datos de EPUB 2 o 3). Sin ISBN en los datos, el programa lo busca en el texto de las primeras y las últimas páginas, donde está la página de créditos.</li>
  <li><b>MOBI y AZW3</b> (Kindle): título, autor, ISBN, editorial y fecha, de la cabecera del archivo.</li>
  <li><b>PDF</b>: título y autor de los datos del documento, y el ISBN buscado en el texto de las primeras y las últimas páginas. Los títulos que los programas ponen solos (<code>Microsoft Word - final.docx</code>, <code>Sin título</code>) se ignoran.</li>
  <li><b>FB2</b>: título, autores, serie, editorial, año e ISBN.</li>
  <li><b>DjVu</b>: solo el nombre del archivo.</li>
  <li><b>Nombre del archivo</b>: entiende <code>Autor - Título</code> y <code>Título - Autor</code> (la búsqueda descubre cuál es cuál), <code>Título by Autor</code>, el año entre paréntesis (<code>(1899)</code>, <code>(2015, Editorial)</code>), el número en la serie (<code>Autor - 03 - …</code>) y el ISBN. Marcas como <code>z-lib.org</code>, <code>libgen</code>, <code>epub</code> y <code>retail</code> se ignoran, y <code>_</code> se convierte en espacio.</li>
</ul>
<ul>
  <li><b>Plantilla</b>: <code>{autor}</code> (el primero), <code>{autores}</code> (todos, separados por &amp;), <code>{autor invertido}</code> (por el apellido: <i>Apellido, Nombre</i>), <code>{inicial}</code> (la primera letra del apellido), <code>{título}</code>, <code>{título original}</code>, <code>{año}</code> (de la primera publicación de la obra), <code>{año de edición}</code>, <code>{editorial}</code>, <code>{isbn}</code>, <code>{serie}</code>, <code>{volumen}</code> (el número en la serie, con dos dígitos: <code>03</code>, para que los libros queden en orden) e <code>{id}</code> (de Open Library). La predeterminada es <code>{autor} - {título} ({año})</code>. Los valores que faltan no dejan restos: sin serie, <code>{serie} {volumen} - {título}</code> queda solo con el título.</li>
  <li><b>Más variables</b>: <code>{idioma}</code> (<code>pt</code>, <code>en</code>) y <code>{páginas}</code> (de Open Library: las de la edición hallada por el ISBN, si no, el número más común entre las ediciones). Limitación: el traductor no está disponible, porque Open Library no lo da.</li>
  <li><b>Lo que ya está al lado</b>: un <code>.opf</code> con el nombre del libro, o el <code>metadata.opf</code> de una carpeta que solo tiene ese libro (como los que graban Renome y los gestores de bibliotecas), vale más que los datos de dentro del archivo: título, autores, ISBN, año, editorial, idioma y serie vienen de él.</li>
  <li><b>Partes opcionales</b>: una parte entre <code>&lt; &gt;</code> solo aparece cuando todas las variables dentro de ella tienen valor (los signos <code>&lt; &gt;</code> no van al nombre). <code>{autor} - &lt;{serie} {volumen} - &gt;{título}</code> da <code>Autor - Serie 03 - Título</code> para libros de una serie y <code>Autor - Título</code> para los demás. También vale en la plantilla de carpetas.</li>
  <li><b>Alternativa</b>: <code>{a|b}</code> pone la primera de esas variables que tenga valor. <code>{título|año}</code> da el título, o el año cuando no hay título. Puede tener más de dos (<code>{a|b|c}</code>) y funciona dentro de una parte opcional.</li>
  <li><b>Cifras</b>: <code>{variable:N}</code> escribe los números con N cifras. <code>{a:3}</code> da <code>005</code> en vez de <code>05</code>, y <code>{a:1}</code> da <code>5</code>, sin ceros delante. Los decimales (<code>12.1</code>) y el disco antes de la pista (<code>1-05</code>) quedan como están; el texto sin número no cambia.</li>
  <li><b>Intercambiar título y autor</b>: el botón ⇄ de la revisión intercambia los dos campos, para nombres leídos al revés.</li>
  <li>El <b>título</b> es el de la edición en el idioma elegido en Opciones (<b>Idioma de los títulos</b>). Cuando el archivo trae otro título, de otra traducción, queda el del archivo. El título original aparece en la revisión y en <code>{título original}</code>.</li>
  <li>El mismo libro en varios formatos (<code>.epub</code>, <code>.pdf</code>, <code>.mobi</code>) con los mismos datos hace una sola búsqueda y recibe la misma elección.</li>
</ul>
<h4>Carpetas, portada y datos</h4>
<ul>
  <li><b>Organizar en carpetas de autor y libro</b>: cada libro va a <code>{autor}/{título}</code> (la plantilla de las carpetas se puede cambiar; cada <code>/</code> es un nivel), como espera Audiobookshelf. En bibliotecas grandes, <code>{inicial}/{autor invertido}/{título}</code> da <code>A/Apellido, Nombre/Título del Libro</code>. Un punto al final del nombre de una carpeta (<i>Apellido, C. S.</i>) se quita, porque Windows no acepta carpetas que terminen en punto. Las carpetas que ya tienen el nombre del autor o del libro se aprovechan, sin crear carpetas nuevas dentro. Una carpeta que solo tiene un libro (en uno o más formatos) cuenta como la suya, tenga el nombre que tenga: las carpetas nuevas quedan junto a ella, no dentro. Las carpetas que queden vacías se borran; deshacer lo devuelve todo a su sitio.</li>
  <li><b>Guardar la portada</b>: guarda la portada de Open Library (o, si no tiene, la de Google Books) como <code>cover.jpg</code> en la carpeta del libro; sin la opción de carpetas, con el nombre del archivo del libro (<code>….jpg</code>).</li>
  <li><b>Guardar los datos del libro</b>: guarda <code>metadata.opf</code> (o <code>….opf</code>, sin la opción de carpetas), en formato OPF, con título, autores, sinopsis, temas, editorial, año, idioma, ISBN y serie. Audiobookshelf y los gestores de libros electrónicos leen este archivo.</li>
  <li>Los libros que ya tienen el nombre correcto también reciben la portada y los datos (cuentan en el botón <b>Renombrar</b>). Los archivos que ya existen no se reemplazan, y deshacer borra lo creado. Sin la opción de abajo, el archivo del libro nunca cambia.</li>
  <li><b>Grabar los datos dentro del EPUB</b>: título, autores, año, editorial, idioma, ISBN y serie van a los datos de dentro del propio EPUB (el archivo OPF que trae), que muestran los lectores y gestores de libros electrónicos. Solo se cambian los datos que cambian; el resto queda como estaba: sinopsis, temas, ilustradores, traductores, la portada y el identificador propio del libro (con el que lectores como Kobo lo reconocen). Un ISBN nuevo entra al lado de ese identificador, sin ocupar su lugar. La serie se graba de las dos formas más leídas (la antigua, y la de EPUB 3 en los EPUB 3). El texto, las imágenes y el resto del libro se copian tal como están. Los libros que ya tienen el nombre correcto también reciben los datos, y los que ya tienen los mismos se saltan.</li>
  <li>La grabación se hace en una copia, que ocupa el lugar del libro solo al final; si el programa se cierra a mitad, lo ya grabado va al <b>Historial</b>. Deshacer devuelve los datos antiguos exactamente como eran. Como el contenido cambia, un torrent ya no reconoce esos archivos.</li>
</ul>
<h4>Volúmenes que faltan</h4>
<p>Con <b>Mostrar los volúmenes que faltan de la serie</b> (activada por defecto), después de la búsqueda, en los libros de una serie la lista muestra filas atenuadas para los volúmenes que faltan <b>entre los suyos</b>, del 1 al mayor número que tiene, como <code>3</code>, en su lugar. Son solo un aviso: no entran en conteos ni en el renombrado. <b>Haga clic en el total</b>, en la barra de abajo, para guardar la lista en un archivo CSV (serie y volumen).</p>
<p>Ningún catálogo tiene una lista fiable de los libros de cada serie, por eso el programa solo ve los huecos entre sus archivos: lo que viene después de su último volumen no aparece. La serie viene de los datos del libro (EPUB, FB2); el número, de ellos o del nombre del archivo (<code>Autor - 03 - Título</code>). Los libros sin serie en sus datos no entran. Volúmenes como <code>2.5</code> (relatos entre dos libros) no llenan ni abren huecos.</p>
<h4>Limitaciones</h4>
<ul>
  <li><b>PDF</b>: muchos PDF tienen los datos del documento vacíos o erróneos (el nombre del archivo de Word, el nombre del equipo de quien lo hizo). Un PDF <b>escaneado</b> es solo una serie de imágenes, sin texto donde buscar el ISBN. En esos casos queda el nombre del archivo: un nombre claro, como <code>Autor - Título</code>, ayuda mucho; uno confuso lleva a la revisión. Conviene arreglar el nombre de esos archivos con las otras reglas, puestas encima de esta en la lista: la búsqueda lee el nombre tal como sale de ellas.</li>
  <li><b>Sin ISBN</b>, el libro se encuentra por el título y el autor, pero no se sabe qué edición es: el nombre usa la editorial y el ISBN del propio archivo, si los tiene, y <code>{año de edición}</code> queda vacío.</li>
  <li><b>Open Library</b> lo mantienen voluntarios. Los libros recientes, de editoriales pequeñas o de algunos países pueden no estar, o estar sin portada, sin año o repetidos, con datos algo distintos en cada copia. Faltan muchas traducciones: cuando el título no aparece, la revisión muestra los otros libros del autor, donde la obra puede estar con el título original, o no estar. En ese caso, marque <b>Ninguno de estos</b> y renómbrelo a mano.</li>
  <li><b>BrasilAPI</b> solo ayuda con libros con ISBN, casi siempre de editoriales brasileñas. Suele venir sin portada y sin año. El año de la primera publicación y la portada vienen de Open Library cuando tiene la obra; si no, <code>{año}</code> queda vacío y no hay portada. Entre los autores pueden venir el traductor o el ilustrador: cuando el archivo dice quién es el autor, solo queda él.</li>
  <li><b>Google Books</b> necesita su clave y acepta unas 1.000 consultas por día; cada libro gasta de una a cuatro. Agotada la cuota, el programa sigue solo con Open Library y BrasilAPI hasta el día siguiente. Sus respuestas se guardan solo mientras el programa está abierto: los términos de Google no permiten guardarlas en el disco. No da la serie ni el año de la primera publicación, y a veces junta ediciones distintas en un solo registro, con datos mezclados; las portadas pueden faltar o ser pequeñas. Un libro encontrado solo allí, por el título, queda sin <code>{año}</code>, y su editorial y su ISBN no se usan, porque no se sabe si el archivo es esa edición.</li>
  <li>A veces el <b>ISBN guardado en el archivo es de otro libro</b> (el de un anuncio al final, o el de la caja de una colección), o el catálogo tiene un error. Cuando el título y el autor encontrados no coinciden con los del archivo, el libro no se elige solo: va a la revisión.</li>
  <li><code>{año}</code> es el de la primera publicación de la obra: un libro publicado en 1937 queda con 1937, aunque la edición sea de 2019. Cuando el catálogo da un año posterior al de la edición guardada en el archivo, lo que es un error suyo, <code>{año}</code> queda vacío. El año de su edición (<code>{año de edición}</code>) solo se conoce cuando el libro se encuentra por el ISBN.</li>
  <li>La <b>serie</b> y el <b>volumen</b> vienen solo de los datos del archivo (de EPUB o de FB2) o del número en el nombre; Open Library casi nunca los tiene.</li>
  <li>Los cómics (CBZ, CBR) y las revistas no los trata esta regla.</li>
  <li>Los datos solo se <b>graban dentro de EPUB</b>. MOBI, AZW3, PDF y FB2 quedan como están (el <code>metadata.opf</code> al lado sirve para ellos). Un EPUB dañado no se cambia y aparece en la lista de errores. Un EPUB con DRM también se salta, porque el lector podría rechazar el libro cambiado.</li>
</ul>
<p class="tip">A Open Library y a Google Books solo se envían el título, el autor y el ISBN buscados, y a BrasilAPI solo el ISBN, nunca el nombre completo del archivo, las carpetas ni el contenido del libro.</p>`,
  },
  {
    id: "r-comic",
    title: "Cómic",
    sub: true,
    html: `
<p>Da a cada cómic un nombre con la serie y el número. Funciona de dos formas: <b>sin internet</b>, con lo que dice el propio archivo, y, si quiere, <b>buscando la serie en un catálogo</b> para completar el nombre, el año, la editorial y el título de cada número.</p>
${ex("Nombre_de_la_Serie_012_(2019)_(Digital)_(grupo).cbz", "Nombre de la Serie #012 (2019).cbz")}
<div class="notice">
<p><b>Antes de usarla: el nombre de los archivos importa</b></p>
<p>Sin internet, esta regla solo sabe lo que cada archivo dice de sí mismo: su <code>ComicInfo.xml</code>, cuando existe, y su nombre. Entiende las formas habituales de escribir la serie, el volumen y el número, pero no adivina lo que el nombre no dice. Una abreviatura (<code>NmSr 12</code>) o una historia numerada sin el nombre de la serie (<code>07 - Nombre de la Historia</code>) no pueden convertirse en <code>Nombre de la Serie #007</code>.</p>
<p>Para un buen resultado, cada archivo necesita al menos:</p>
<ul>
  <li>el <b>nombre de la serie</b> completo, escrito siempre igual;</li>
  <li>el <b>número</b> (<code>#12</code>, <code>012</code>) y, si la serie tuvo más de una etapa, el <b>volumen</b> (<code>Vol. 2</code>);</li>
  <li>el título o el año, si quiere conservarlos, después de " - " o entre paréntesis.</li>
</ul>
<p>Nombres así funcionan bien: <code>Nombre de la Serie #012</code>, <code>Nombre de la Serie Vol. 2 #005</code>, <code>Nombre de la Serie - Nombre del Álbum (1990)</code>.</p>
<p>Si sus archivos están lejos de eso, ponga encima de esta regla, en la lista, las reglas que arreglan el nombre: <b>Reemplazar texto</b> para cambiar abreviaturas por el nombre completo, <b>Reorganizar partes</b> o <b>Expresión regular</b> para cambiar el orden, <b>Quitar fragmento</b> para quitar restos. La regla <b>Cómic</b> y la búsqueda leen el nombre tal como sale de esas reglas, y la columna <b>Nuevo nombre</b> muestra el resultado antes de renombrar.</p>
<p>Dos atajos: para una carpeta con una sola serie, la opción <b>La serie es el nombre de la carpeta</b> no necesita otras reglas; y, buscando en un catálogo, el campo <b>Serie a buscar</b> encuentra la serie correcta aunque el nombre esté abreviado.</p>
</div>
<h4>Sin internet</h4>
<ul>
  <li>En <b>CBZ, CBR, CB7 y CBT</b>, el programa lee el <code>ComicInfo.xml</code> de dentro del archivo, cuando existe: serie, número, volumen, título, año, editorial, guion y dibujo. Es el dato más fiable, con una excepción: si la serie del <code>ComicInfo.xml</code> es otra que la del nombre del archivo (el original en inglés de una edición traducida, por ejemplo), valen la serie y el título del nombre, que es como usted llamó al archivo.</li>
  <li>Sin él (y siempre en <b>PDF, EPUB y libros de Kindle</b>), usa el nombre del archivo. Entiende el número (<code>Serie 012</code>, <code>#12</code>, <code>nº 12</code>, <code>Número 12</code>, <code>#12.1</code>), el volumen o tomo (<code>v02</code>, <code>Vol. 3</code>, <code>Vol. III</code>, <code>(Vol.03)</code>, <code>Tomo 5</code>, <code>T05</code>, <code>Libro 3</code>), el capítulo de manga (<code>Cap. 21</code>, <code>Ch. 21</code>, <code>c045</code>), el año entre paréntesis y el total (<code>(de 6)</code>, <code>#2 de 6</code>). Lo que hay entre el volumen y el número es de la serie (<code>Serie Vol. 2 Anual #01</code> es el anual, no el número 1). Un número seguido de una palabra es parte del nombre (<code>Nombre con 714 Palabras</code>, <code>10 Años</code>), y un número al principio seguido de un guion es el del número (<code>19 - Nombre de la Historia</code>). Las marcas como <code>(Digital)</code>, <code>(c2c)</code> y los nombres de grupos entre corchetes o paréntesis se ignoran.</li>
  <li>Un archivo solo con el número (<code>012.cbz</code>) toma la serie de su carpeta; una carpeta <code>Nombre de la Serie (2012)</code> también da el año de la serie, y una carpeta <code>Vol. 3</code>, el volumen.</li>
  <li>Un nombre sin espacios, con las palabras pegadas (<code>CapitanAuroraEspecial012.cbz</code>), se separa donde una mayúscula sigue a una minúscula y donde un número sigue a una palabra: <code>Capitan Aurora Especial 012</code>. Nombres como <code>McLane</code> y una minúscula al inicio (<code>iHeroe</code>) quedan juntos. Limitación: siglas pegadas (<code>XMenAnual</code>) pueden separarse en el lugar equivocado; corríjalo en la ventana de elección o con <b>Editar los datos</b>.</li>
  <li>En un nombre que empieza por su número (<code>19 - Nombre de la Historia</code>), el número es el del principio: los números dentro del título no cuentan.</li>
  <li><b>La serie es el nombre de la carpeta</b>: para una carpeta de una sola serie, con archivos que traen el número y el título de la historia (<code>Nombre de la Serie (Completo)/01 - Nombre de la Historia</code>). Cada archivo queda con la serie de la carpeta (sin el año y lo que está entre paréntesis), y lo que traía su nombre pasa a ser el título: <code>Nombre de la Serie #001 - Nombre de la Historia</code>. Si el nombre solo repite la serie, o una parte de ella (<code>SERIE v01</code> en una carpeta con el nombre original y el traducido), no hay título. Viene desactivada, porque en una carpeta que no es de una serie (<code>Descargas</code>) su nombre pasaría a ser la serie de todos.</li>
  <li>Un número único o recopilatorio sin numeración queda con la serie y el año (<code>Nombre del Álbum (2024)</code>), sin las marcas del nombre original.</li>
  <li>Algunos programas guardan las letras acentuadas como su código entre guiones bajos (<code>M_244_nica</code> por <i>Mônica</i>); el programa las restituye.</li>
  <li>Los archivos en los que no se encuentra ni la serie quedan como están.</li>
</ul>
<h4>Buscando en un catálogo</h4>
<p>En <b>Dónde buscar</b>, elija el catálogo:</p>
<ul>
  <li><b>Grand Comics Database</b> (comics.org): hecho por voluntarios, con ediciones de muchos países. Da el nombre de la serie como se publicó, el año, la editorial y la numeración. Funciona mejor con una <b>cuenta gratuita</b> de comics.org, puesta en <b>Opciones → Grand Comics Database</b>. Sin cuenta, el GCD acepta solo unas 30 consultas por hora, y puede dejar de aceptarlas; la primera búsqueda sin cuenta lo avisa.</li>
  <li><b>Metron</b> (metron.cloud): hecho por voluntarios, fuerte en cómics estadounidenses, con cada número y su código de barras. Pide el <b>token</b> de una cuenta gratuita de Metron, pegado en <b>Opciones → Metron</b>. Solo en él se puede encontrar el número por el <b>código de barras</b> (vea abajo).</li>
  <li><b>Comic Vine</b> (comicvine.gamespot.com): fuerte en cómics estadounidenses, con el título de cada número. Pide una clave gratuita, pegada en <b>Opciones</b>.</li>
  <li><b>MangaDex</b> (mangadex.org): manga, sin cuenta ni clave. Da el nombre y el año de la serie (en su idioma, cuando lo hay, o en inglés) y el autor; el volumen y el capítulo quedan los del archivo.</li>
  <li><b>AniList</b> (anilist.co): manga, sin cuenta ni clave. Da el nombre, el año, el autor y el número de tomos de la serie; el volumen y el capítulo quedan los del archivo.</li>
  <li><b>MangaUpdates</b> (mangaupdates.com): manga, sin cuenta ni clave. Da el nombre, el año, el autor, la editorial original y el número de tomos de la serie; el volumen y el capítulo quedan los del archivo. Los datos vienen de MangaUpdates.</li>
</ul>
<p>En AniList y MangaUpdates, cuando el nombre del archivo es uno de los nombres de la serie (una traducción, por ejemplo), la serie queda con ese nombre; si no, con el nombre en inglés (AniList) o el principal, a menudo en japonés romanizado.</p>
<ol>
  <li>Haga clic en <b>Buscar la serie</b>. Los archivos se agrupan por serie (o use <b>Serie a buscar</b> para todos a la vez). Cuando solo una serie tiene ese nombre y el año de los archivos (o es la única encontrada) y números suficientes para los de ellos, se elige sola; sin el año el nombre no basta (una traducción de una serie nueva puede llevar el nombre de una antigua), y usted elige; si no, usted elige entre las publicaciones, que muestran país, idioma, año de inicio y cantidad de números. Si el nombre completo no encuentra nada (<i>Serie: Historia</i>), la búsqueda prueba solo la serie.</li>
  <li>En la ventana de elección puede buscar de nuevo con otro nombre y acotar la lista con el <b>Año</b> en que empezó la serie y la <b>Editorial</b> (basta parte del nombre; los catálogos de manga no se buscan por editorial). Las series con menos números que los de los archivos van al final. El GCD compara el nombre letra por letra: mayúsculas y acentos no cuentan, pero la puntuación sí, por eso el programa busca el nombre tal como se escribió y también sin la puntuación (<i>Serie: Nombre</i> encuentra también <i>Serie Nombre</i>). Con el año, el GCD se consulta por la palabra más larga del nombre, y la puntuación deja de estorbar: <i>Serie Nombre</i> encuentra también <i>Serie: Nombre</i>. En la editorial vale el nombre corto que usa el catálogo (<i>Titan</i> se encuentra con <i>Titan Comics</i>), y se pueden poner varias, separadas por <code>;</code>. En el GCD, la editorial cuesta una consulta más por cada editorial distinta encontrada (hasta 12, las más frecuentes primero): añada el año para que sean pocas, sobre todo sin cuenta.</li>
  <li><b>Editoriales a ignorar</b> (en la regla): las series de estas editoriales nunca entran en las búsquedas, ni en la automática ni en la ventana de elección; solo por enlace pegado. Útil cuando una editorial con series del mismo nombre siempre aparece en lugar de la correcta. Separe con <code>;</code>; basta parte del nombre, como en el campo Editorial. Limitación: MangaDex y AniList no dicen la editorial, así que no se quita nada de ellos.</li>
  <li>Cada archivo se empareja con su número por la numeración (o por el volumen, en un manga numerado por volúmenes; un número único, con el único número de la serie). Los que no coinciden quedan con el nombre de la serie y su propio número, en la misma carpeta que los demás, y abren la revisión, con la lista de números de la serie, para elegir el número exacto. <b>Ninguno de estos</b> devuelve el nombre que dice el archivo.</li>
  <li>La revisión muestra la <b>portada del archivo</b> (su primera página; en un PDF, la primera página dibujada) y la compara con la portada de cada número: los de portada igual suben al principio con la marca <b>misma portada</b>, que vale también para el mismo arte en otro idioma. Con hasta 60 números con portada, la comparación se hace sola; con más, pulse <b>Comparar portadas</b> (los 300 primeros; cada portada se descarga una vez por sesión). Una portada variante, o una página de apertura antes de la portada, no coinciden.</li>
  <li>En <b>Elegir la serie</b>, la portada del archivo aparece en la esquina y se compara con la del mismo número (o volumen) en cada una de las 9 primeras series: las que coinciden suben con la marca <b>misma portada</b>, cuando todas se han comparado. Así se distingue la publicación correcta de otra con el mismo nombre. Funciona con Metron, Comic Vine y MangaDex (una o dos consultas más por serie; las series de más de 200 números quedan fuera); con AniList y MangaUpdates, que solo tienen la portada de la serie, solo para el volumen 1. <b>Grand Comics Database no entrega sus portadas a los programas</b> (ni en la lista de series ni en los números): con él no hay comparación de portadas. Haga clic en una portada (la del archivo, la de una serie o la de un número) para verla grande, junto a la del archivo; Esc o un clic la cierran. Las portadas aparecen poco a poco: cada serie pide al catálogo su lista de números y después descarga la portada, así que el tiempo depende de lo rápido que responde el sitio y del intervalo que exige entre consultas (el programa espera 3 segundos entre las de Metron, cerca de 1 segundo en Comic Vine y medio segundo en MangaDex). Con Metron, comparar las 9 series puede llevar medio minuto; mientras tanto, ya se puede elegir.</li>
</ol>
<ul>
  <li><b>Plantilla lista</b>: <b>Predeterminada de Renome</b> (que sirve también para Komga, que toma cada carpeta como una serie) o <b>Kavita</b> (sin el título después de " - ", que Kavita no lee). Las dos rellenan la plantilla y las carpetas y activan las carpetas de serie, porque los dos servidores quieren cada serie en su carpeta. Después se pueden cambiar libremente.</li>
  <li><b>Plantilla</b>: <code>{serie}</code>, <code>{año de la serie}</code>, <code>{número}</code> (tres dígitos: <code>012</code>), <code>{volumen}</code> (dos), <code>{capítulo}</code> (tres), <code>{título}</code> (del número), <code>{año}</code> (del número), <code>{total}</code>, <code>{editorial}</code>, <code>{guion}</code>, <code>{dibujo}</code> e <code>{id}</code> (del catálogo). La predeterminada es <code>{serie} v{volumen} #{número} c{capítulo} - {título} ({año})</code>: lo que falta se va junto con la marca pegada a ello (<code>v</code>, <code>#</code>, <code>c</code>), terminada en punto (<code>Vol. </code>) o el guion anterior. Así, un número estadounidense queda <code>Serie #012 (2019)</code>, un volumen de manga, <code>Serie v03</code>, y un tomo recopilatorio con título, <code>Serie v01 - Título (2018)</code>.</li>
  <li><b>Más variables</b>, del <code>ComicInfo.xml</code>: <code>{arco}</code>, <code>{sello}</code>, <code>{formato}</code>, <code>{género}</code>, <code>{entintador}</code>, <code>{colorista}</code>, <code>{rotulista}</code>, <code>{portadista}</code>, <code>{editor}</code>, <code>{traductor}</code>, <code>{páginas}</code>, <code>{edad}</code>, <code>{idioma}</code>, <code>{serie alternativa}</code>, <code>{número alternativo}</code>, <code>{gtin}</code>, y la fecha de portada (la del catálogo, cuando el número se encuentra allí). Fechas: <code>{fecha}</code> (<code>2010-05-03</code>), <code>{mes}</code> (<code>05</code>), <code>{día}</code> y el nombre del mes en el idioma de la variable: <code>{nombre del mes}</code> da <code>mayo</code>, <code>{month name}</code> da <code>May</code> y <code>{nome do mês}</code> da <code>maio</code>. <code>{scan}</code> es quien lo digitalizó: el del <code>ComicInfo.xml</code>, si no, la última nota entre paréntesis del nombre que no es año ni palabra conocida (<code>(Grupo)</code>), o lo que sigue a <code>--</code>. Lo que sigue a <code>--</code> o <code>__</code> nunca entra en la serie, y <code>c2c</code> y <code>FCBD</code> se ignoran.</li>
  <li><b>Otros formatos de datos</b>: sin <code>ComicInfo.xml</code>, se lee el <code>MetronInfo.xml</code> de dentro del archivo (serie, número, volumen, historias, fecha de portada, editorial y sello, creadores por función, arco, páginas, clasificación, idioma, GTIN y el ID de Comic Vine); sin ninguno de los dos, los datos que algunos programas guardan en el comentario del zip (ComicBookInfo).</li>
  <li><b>Partes opcionales</b>: una parte entre <code>&lt; &gt;</code> solo aparece cuando todas las variables dentro de ella tienen valor (los signos <code>&lt; &gt;</code> no van al nombre). <code>{serie} #{número}&lt; - {guion}&gt;</code> solo pone el guionista cuando se conoce. También vale en la plantilla de carpetas.</li>
  <li><b>Alternativa</b>: <code>{a|b}</code> pone la primera de esas variables que tenga valor. <code>{título|año}</code> da el título, o el año cuando no hay título. Puede tener más de dos (<code>{a|b|c}</code>) y funciona dentro de una parte opcional.</li>
  <li><b>Cifras</b>: <code>{variable:N}</code> escribe los números con N cifras. <code>{a:3}</code> da <code>005</code> en vez de <code>05</code>, y <code>{a:1}</code> da <code>5</code>, sin ceros delante. Los decimales (<code>12.1</code>) y el disco antes de la pista (<code>1-05</code>) quedan como están; el texto sin número no cambia.</li>
  <li><b>Organizar en carpetas de serie</b>: cada cómic va a <code>{serie} ({año de la serie})</code> (la plantilla de las carpetas se puede cambiar; cada <code>/</code> es un nivel). Un cómic que ya está en una carpeta con el nombre de su serie se queda donde está, aunque la carpeta tenga otro formato (<code>Nombre de la Serie Vol. 2 (1988-1998)</code>, <code>Nombre de la Serie/Vol. 1</code>, <code>Nombre de la Serie</code> para un especial <code>Nombre de la Serie - Especial</code>, o una grafía un poco distinta). Una carpeta en la que todos los cómics son de una sola serie cuenta como la carpeta de la serie, tenga el nombre que tenga (<code>Mis cómics</code>): la carpeta nueva queda junto a ella, no dentro, y lo que quede en ella sigue la regla de las carpetas con sobrantes (vea Vista previa y conflictos). Los cómics de la lista se organizan juntos, como una biblioteca:<ul><li>cada serie queda con una sola grafía, la más común en la lista (mayúsculas, acentos y una letra cambiada en una palabra, como en <code>Apocalipse</code> y <code>Apocalypse</code>, cuentan como la misma serie), en el nombre de la carpeta y en el de los archivos;</li><li>un especial o número único va a la carpeta de la serie con cuyo nombre empieza el suyo (<code>Nombre de la Serie - Especial</code> en <code>Nombre de la Serie</code>; si hay varias, la de nombre más largo);</li><li>los números únicos con la misma parte antes del " - " (<code>Nombre de la Serie - Álbum Uno</code>, <code>Nombre de la Serie - Álbum Dos</code>) van juntos a una carpeta con esa parte;</li><li>los demás van cada uno a la carpeta de su serie, aunque sea un solo archivo.</li></ul>Deshacer lo devuelve todo a su sitio.</li>
</ul>
<h4>Código de barras (Metron)</h4>
<p>Con <b>Metron</b>, un cómic que tiene el código de barras grabado en el <code>ComicInfo.xml</code> (el campo GTIN) o en el <code>MetronInfo.xml</code> se encuentra directamente por él, con el número exacto, sin pasar por la elección de la serie. Con <b>Leer el código de barras impreso en las páginas</b>, el programa también busca el código en la imagen de la primera y las dos últimas páginas (la portada y la contraportada) de los cómics que no lo tienen grabado: el UPC de los estadounidenses, con los 5 dígitos de más que dicen el número y la portada, o el ISBN de un tomo recopilatorio. Un código leído así también va al <code>ComicInfo.xml</code> cuando se graban los datos. Los que no se encuentran por el código siguen la búsqueda por el nombre.</p>
<h4>Grabar los datos en el archivo</h4>
<p>Con <b>Grabar los datos en el archivo (ComicInfo.xml)</b>, después de renombrar, cada CBZ emparejado con un número del catálogo recibe dentro un <code>ComicInfo.xml</code> con serie, número, título, total de números, volumen, fecha de portada, guion, dibujo, editorial, la página del número en el catálogo y lo demás que el archivo ya decía (arco, sello, colorista…). Es el archivo que leen Komga, Kavita y los lectores de cómics. Si el CBZ ya tenía uno, solo se cambian los campos que cambian: la sinopsis, la lista de páginas y el resto se quedan. Las páginas se copian tal como están, sin perder calidad. Los cómics que ya tienen el nombre correcto también reciben los datos (cuentan en el botón <b>Renombrar</b>), y los que ya tienen los mismos se saltan. Los encontrados en MangaDex, AniList y MangaUpdates quedan marcados como manga.</p>
<p>La grabación se hace en una copia, que ocupa el lugar del archivo solo al final; si el programa se cierra a mitad, lo ya grabado va al <b>Historial</b>. Deshacer devuelve el <code>ComicInfo.xml</code> antiguo, o quita el que se puso. Como el contenido cambia, un torrent ya no reconoce esos archivos.</p>
<h4>Editar los datos a mano</h4>
<p>Para lo que ningún catálogo encuentra (ediciones nacionales, fanzines, escaneos raros), seleccione los cómics en la lista y haga clic en <b>Editar los datos</b>, en la regla. En la ventana, escriba serie, año de la serie, volumen, número, título, año, editorial, guion y dibujo. Lo que escriba vale en lugar de lo que dicen el archivo y el catálogo: el nombre nuevo se hace con eso y, con <b>Grabar los datos</b>, va al <code>ComicInfo.xml</code> (sin dirección de catálogo). Con varios seleccionados, solo los campos que cambie valen para todos (útil para corregir la serie o la editorial de una vez); un campo con valores distintos aparece vacío y queda como lo tiene cada uno. Limitaciones: los datos escritos valen mientras el programa esté abierto y los archivos en la lista (como las búsquedas); <b>Limpiar</b> los olvida y Ctrl+Z no los deshace; otros campos (arco, colorista, sinopsis) no se editan aquí.</p>
<h4>Borrar el ComicInfo.xml</h4>
<p>Con <b>Borrar el ComicInfo.xml de los CBZ</b> (y <b>Grabar los datos</b> desactivada), cada CBZ de la regla que tenga un <code>ComicInfo.xml</code> queda sin él después de renombrar. Sirve para limpiar datos erróneos grabados por otro programa. El nombre nuevo se hace antes, todavía con lo que decía. Con <b>Grabar los datos</b> activada, la opción pasa a ser <b>Grabar desde cero</b>: el <code>ComicInfo.xml</code> de los cómics encontrados en el catálogo queda solo con lo que dice el catálogo (la lista de páginas se queda); los no encontrados quedan como están, porque lo que dicen es lo que les da el nombre. Deshacer devuelve el antiguo. Limitaciones: solo sale el <code>ComicInfo.xml</code>; el <code>MetronInfo.xml</code> y los datos en el comentario del zip (ComicBookLover) se quedan; un cómic convertido a CBZ no pasa por esta limpieza.</p>
<h4>Convertir a CBZ</h4>
<p>Con <b>Convertir CBR, CB7, CBT, PDF, EPUB, MOBI/AZW3 y carpetas de imágenes a CBZ</b>, cada cómic en RAR, 7z o TAR pasa a ser un CBZ, el formato que todo lector abre. La extensión cambia ya en la vista previa (<code>Nombre.cbr</code> → <code>Nombre.cbz</code>), y los <code>.cbz</code> que en realidad son RAR o 7z también se convierten, aunque no cambien de nombre (cuentan en el botón <b>Renombrar</b>). Las páginas se copian tal cual, en el mismo orden y sin perder calidad. Un <b>PDF</b> se convierte página por página, siempre de la manera que más conserva. Cuando la página es una sola imagen, como en un cómic escaneado, la imagen sale tal cual: JPEG sigue siendo JPEG; las demás pasan a PNG, sin pérdida. Eso vale también para imágenes en JPEG 2000, JBIG2 y fax, que se decodifican sin pérdida y se graban en PNG. Las imágenes en CMYK (de imprenta) pasan a RGB, con los colores aproximados. Lo que se dibuja debajo de la imagen, como el fondo y el marco de una página web impresa en PDF, queda fuera, siempre que la imagen cubra casi toda la página. Una página que el PDF muestra girada o reflejada sale derecha: un JPEG se gira sin pérdida cuando sus medidas lo permiten (casi siempre); si no, pasa a PNG, más grande, pero también sin pérdida, y una página vacía pasa a ser una página blanca del tamaño de las demás. Solo cuando nada de eso sirve (texto y dibujo de verdad, páginas en capas, varias imágenes en una página, sellos y comentarios encima) la página se <b>dibuja</b> como la muestra un lector, a la resolución de su imagen más nítida (o, si no tiene imágenes, al <b>Tamaño de las páginas dibujadas</b>: 1600, 2400 o 3200 píxeles en el lado mayor), y se graba en PNG o, cuando sale mucho menor, en JPEG. Se dibujan varias páginas a la vez, una por núcleo del procesador. En la vista previa, un PDF que tendrá páginas dibujadas recibe la etiqueta <b>N pág. dibujada(s)</b>, y el aviso del final dice cuántas fueron. En <b>Páginas de PDF que no son solo una imagen</b>, <b>No dibujar</b> deja esos PDF como están: solo se convierten los que se pueden convertir sin dibujar nada. </p>
<p>Un <b>EPUB</b> o un libro de <b>Kindle</b> (MOBI, AZW3) se convierte cuando toda página suya es una imagen, como en los cómics vendidos en esos formatos: las imágenes salen tal cual, en orden de lectura, con la portada primero. Un libro con texto (una página con más que un número, o sin imagen) no es un cómic y queda como está, porque el texto se perdería; un EPUB o libro de Kindle con DRM tampoco. Una <b>carpeta solo de imágenes</b>, añadida con <b>Carpetas (renombrar las propias carpetas)</b>, recibe el nombre del cómic y se vuelve un CBZ con sus imágenes (y las de sus subcarpetas, como capítulos), en el orden de los números; un <code>ComicInfo.xml</code> que esté en ella va junto. Una carpeta con cualquier otro tipo de archivo (vídeo, documento) queda como está.</p>
<p>Con <b>Grabar los datos en el archivo</b> activada, el título y el autor del propio PDF, EPUB o libro de Kindle van al <code>ComicInfo.xml</code> cuando el catálogo no los da. Con <b>Grabar los datos en el archivo</b> también activada, el <code>ComicInfo.xml</code> va ya en el CBZ nuevo; sin ella, lo que traía el original va igual.</p>
<p>El CBZ se escribe en una copia al lado y solo ocupa el lugar del original cuando está completo y comprobado. En <b>El archivo original</b> usted elige qué pasa con él. <b>Mantener al lado</b> deja el original en la misma carpeta, con el nombre nuevo y su propia extensión (<code>Nombre.cbr</code>; una carpeta queda con el nombre nuevo, sin extensión); deshacer lo devuelve y borra el CBZ. <b>Eliminar</b> manda el original a la Papelera del sistema, de donde se puede recuperar; deshacer solo devuelve el nombre antiguo al CBZ, que sigue siendo CBZ.</p>
<h4>Números que faltan</h4>
<p>Con <b>Mostrar los números que faltan</b> (activada por defecto), tras la búsqueda la lista muestra filas atenuadas para los números ya publicados de la serie que ningún archivo de la lista tiene, como <code>#7 · Título</code>, con la fecha de portada. Quedan en el lugar del número, entre sus vecinos, y la barra inferior dice cuántos faltan. Son solo un aviso: no se pueden seleccionar y no cuentan para totales ni renombrado. <b>Haga clic en ese total</b> para guardar los números faltantes en un archivo CSV (serie, número, título y fecha de portada); la ventana se abre en la carpeta de la serie. Solo cuentan las series con algún archivo casado con un número.</p>
<h4>Limitaciones</h4>
<ul>
  <li>Los <b>CBR</b> (RAR) se leen con libarchive, una biblioteca de código abierto incluida en el programa. Abre casi todos, pero algunos RAR antiguos dan error (el programa oficial de RAR los abre): esos se quedan como están y aparecen en la lista de errores. Los RAR con contraseña y los divididos en partes (<code>.part1.rar</code>) no se convierten. Los RAR no se pueden escribir: para grabar los datos en un CBR, hay que convertirlo.</li>
  <li>Los nombres de archivo muy fuera de lo común pueden leerse mal (un número que forma parte del nombre de la serie, por ejemplo). Revise la columna <b>Nuevo nombre</b> antes de renombrar.</li>
  <li>Los catálogos tienen huecos: las ediciones brasileñas y de otros países pueden faltar en Comic Vine, y el GCD, hecho por voluntarios, puede estar incompleto. Cuando no esté la serie correcta, sáltela: los archivos quedan con el nombre que dicen ellos mismos. La búsqueda no siempre encuentra una serie por un nombre traducido; en el selector, puede buscar de nuevo con otro nombre (el original, por ejemplo).</li>
  <li>En el GCD, la revisión muestra solo la numeración, sin títulos ni portadas: traerlos costaría una consulta por número. Una serie con el mismo nombre en otro idioma (la edición francesa, cuando hay una en el suyo) no se elige sola.</li>
  <li>El GCD y Comic Vine cuentan <b>números y volúmenes</b>, no capítulos: los capítulos sueltos de manga no se buscan y quedan con el nombre leído del archivo. Metron también. MangaDex, AniList y MangaUpdates solo dan la serie.</li>
  <li><b>Límites de consultas</b>: el GCD acepta unas 25 por minuto con cuenta (unas 30 por hora sin cuenta); Comic Vine, unas 200 por hora, solo para uso no comercial; Metron, unas 20 por minuto y 5.000 por día (una serie con muchos números gasta una consulta más por cada 100 números, y el programa espera 3 segundos entre ellas); AniList, solo uso no comercial y unas 30 por minuto. El programa usa pocas por serie y guarda las respuestas mientras está abierto; si llega a un límite, espere un poco y busque de nuevo.</li>
  <li>Los <b>números que faltan</b> siguen la lista del catálogo: anuales, especiales y números con numeración rara (<code>#1.5</code>, <code>#0</code>) aparecen como faltantes si no los tiene, y una serie con cientos de números, de la que tiene pocos, llena la lista. Desactive la opción en ese caso. La del GCD no tiene fechas de portada.</li>
  <li>El <b>código de barras</b> solo ayuda en Metron, y solo cuando Metron tiene el código de ese número (muchos antiguos y los brasileños no lo tienen). Muchos archivos no traen la contraportada, o la traen con el código cortado, borroso o demasiado pequeño; los PDF no se leen (habría que dibujar las páginas). Un código que sirve para varios números (el UPC sin los 5 dígitos de más, que usaban las ediciones de quiosco) no dice el número y se ignora. Leer las páginas tarda de medio segundo a unos segundos por archivo, así que en una colección grande tarda. Rara vez, una marca en la imagen puede leerse como un código válido de otro número: revise la vista previa.</li>
  <li><b>MangaDex</b>, <b>AniList</b> y <b>MangaUpdates</b> no tienen la lista de tomos de un manga (solo de los capítulos que subieron los lectores, y los de mangas licenciados suelen haberse retirado). Por eso, con él, los que faltan son solo los que faltan <b>entre sus archivos</b>, del 1 al número más alto que tiene (por tomo cuando todos los archivos tienen uno; si no, por número o capítulo): lo que viene después del último no aparece.</li>
  <li>El <b>GCD</b> registra cada portada variante como un número aparte con el mismo número; el programa se queda con uno por número (la portada normal, cuando la hay), en la cuenta, la revisión y los números que faltan.</li>
  <li>Los datos solo se <b>graban en CBZ</b> (que sean ZIP por dentro). CBR, CB7, CBT, PDF, EPUB y libros de Kindle los reciben solo si se convierten; los EPUB que no son cómics pueden recibirlos con la regla de libros. Un archivo dañado no se cambia y aparece en la lista de errores. Con las opciones apagadas, el archivo nunca cambia.</li>
  <li>La <b>conversión</b> lee y escribe cada archivo entero: en una colección grande lleva tiempo y necesita espacio libre para una copia cada vez. El CBZ queda más o menos del tamaño del original (a veces algo mayor, porque las páginas no se comprimen de nuevo). Con <b>Mantener al lado</b>, los dos quedan en la carpeta, y Komga y Kavita muestran los dos hasta que usted borre los originales. Con <b>Eliminar</b>, deshacer no trae el original de vuelta: queda en la Papelera hasta que la vacíe. Un archivo que no se pudo convertir (dañado, con contraseña) vuelve a la extensión que tiene de verdad (<code>Nombre.cbr</code>).</li>
  <li>Una página <b>dibujada</b> es una imagen de la página: el texto deja de ser texto, la resolución es fija y el archivo queda mucho mayor que el PDF (un libro de texto de 1 MB puede pasar a un CBZ de 90 MB). Las páginas en capas pierden un poco al unirse. El dibujo lo hace <code>hayro</code>, una biblioteca de código abierto incluida en el programa, que aún no reproduce todo lo del PDF (algunos modos de mezcla de colores y transparencia), y tarda unos segundos por libro. Los PDF protegidos con contraseña no se convierten. Las imágenes guardadas dentro del contenido de la página solo se extraen tal cual cuando no están comprimidas (las demás se dibujan).</li>
  <li>Un <b>EPUB</b> o libro de <b>Kindle</b> con texto no se convierte, ni siquiera un libro ilustrado o un cómic que lleve el texto de los globos sobre las imágenes: el CBZ solo guarda imágenes, y no hay cómo dibujar el texto del libro en ellas. Tampoco se convierten los que tienen varias imágenes en una página (la posición de cada una se perdería), los que tienen DRM (casi todos los comprados en tiendas) y los libros de Kindle con la compresión más fuerte del formato (algunos hechos por el programa de Amazon). Los EPUB del Internet Archive, hechos por reconocimiento de texto, tienen páginas sin imagen y no se convierten. Un AZW3 que quede al lado conserva esa extensión; un MOBI, o un AZW3 con las dos versiones del formato, queda <code>.mobi</code>.</li>
  <li>Una <b>carpeta de imágenes</b> solo se convierte si tiene únicamente imágenes (JPEG, PNG, GIF, WebP, AVIF, JPEG XL), el <code>ComicInfo.xml</code> y los archivos que el sistema crea solo (<code>Thumbs.db</code>, archivos ocultos), en hasta tres niveles de subcarpetas. Una carpeta de fotos también es solo de imágenes: si está en la lista con la regla activada, se vuelve un CBZ, así que revise la vista previa.</li>
  <li>Solo se graba lo que dice el catálogo (o el propio archivo): en el GCD, los números vienen sin título, y los catálogos de manga solo dan la serie. Un <code>MetronInfo.xml</code> que tenga el archivo no se actualiza.</li>
</ul>
<p class="tip">A los catálogos solo van el nombre de la serie buscada y los números de las series y los números, nunca el nombre completo del archivo, las carpetas ni el contenido. El acceso del GCD solo va a comics.org.</p>`,
  },
  {
    id: "r-photo",
    title: "Foto",
    sub: true,
    html: `
<p>Nombra <b>fotos y vídeos</b> de móvil o cámara por la <b>fecha en que se tomaron</b> y, si quiere, los organiza en carpetas por fecha. Funciona sin internet: todo se lee de los propios archivos.</p>
${ex("IMG_4821.JPG", "2019-05-12 14.30.22.JPG")}
${ex("IMG-20190513-WA0001.jpg", "2019-05-13.jpg")}
<h4>De dónde viene la fecha</h4>
<ol>
  <li><b>De dentro del archivo</b>: en las fotos, la fecha del EXIF (JPEG, HEIC, PNG, WebP, TIFF y los RAW basados en TIFF, como DNG, CR2, NEF, ARW, PEF y SRW); en los vídeos MP4, MOV, M4V y 3GP, la fecha de grabación.</li>
  <li><b>Del nombre del archivo</b> (opción activada por defecto): <code>IMG_20190512_143022</code>, <code>PXL_20190512_143022123</code>, <code>VID_20190512_143022</code>, <code>IMG-20190512-WA0001</code> (WhatsApp), <code>Screenshot_2019-05-12-14-30-22</code>, <code>WhatsApp Image 2019-05-12 at 14.30.22</code>, <code>2019-05-12 14.30.22</code>. Un nombre solo con el día deja la hora vacía. En los <b>vídeos</b>, un nombre con fecha y hora (<code>20260913_185946</code>, <code>VID_…</code>) vale más que la fecha de dentro: el móvil pone en el nombre el inicio de la grabación, y dentro del vídeo suele quedar la hora en que se cerró el archivo, segundos o minutos después.</li>
  <li><b>De la fecha de modificación</b>, solo si activa la opción. En una foto copiada, descargada o recibida suele ser la fecha de la copia.</li>
</ol>
<p>Sin ninguna, el archivo <b>queda como está</b> y lleva una marca en la lista; la tarjeta dice cuántos quedaron así. Una fecha tomada del nombre o de la modificación también lleva marca (pase el ratón para ver de dónde vino).</p>
<h4>Plantilla</h4>
<p>Use <code>{año}</code>, <code>{mes}</code>, <code>{día}</code>, <code>{hora}</code>, <code>{minuto}</code>, <code>{segundo}</code>, <code>{fecha}</code> (2019-05-12), <code>{nombre del mes}</code>, <code>{cámara}</code> (marca y modelo, como <code>Apple iPhone 12</code>), <code>{ancho}</code>, <code>{alto}</code> y <code>{nombre}</code> (el nombre actual). Las plantillas listas dan <code>2019-05-12 14.30.22</code>, <code>20190512_143022</code>, con la cámara entre paréntesis, o la fecha antes del nombre actual. Lo que queda vacío (una hora que no se sabe, una cámara que la foto no dice) desaparece junto con los separadores y paréntesis de alrededor.</p>
<h4>Archivos que van juntos</h4>
<p>En una carpeta, los archivos con el <b>mismo nombre antes de la extensión</b> son una sola foto y reciben el mismo nombre nuevo: el JPEG y el RAW (<code>IMG_0001.JPG</code> e <code>IMG_0001.CR2</code>), las ediciones (<code>IMG_0001.xmp</code>, <code>IMG_0001.CR2.xmp</code>, el <code>.AAE</code> del iPhone) y la foto y el vídeo de una Live Photo (<code>IMG_0001.HEIC</code> e <code>IMG_0001.MOV</code>). La fecha es la del archivo que la tiene dentro (la foto antes del RAW y del vídeo).</p>
<h4>Fotos del mismo segundo</h4>
<p>Las fotos en ráfaga, y las fotos cuyo nombre nuevo ya existe en la carpeta, reciben <code>-1</code>, <code>-2</code>… en vez de un conflicto, en el orden de sus nombres actuales; los archivos que van juntos reciben el mismo número. Renombradas así, quedan igual la próxima vez. Las copias idénticas no reciben número: el conflicto avisa que son iguales. En cambio, dos versiones de una misma foto que no son copias exactas (el original de 15 MB y un JPG recomprimido de 500 KB) <b>se llevan las dos</b>, cada una con su número. Para quedarse solo con la mejor, abra <b>Comparar</b> en las imágenes parecidas antes de renombrar y use <b>Conservar solo la mayor</b>.</p>
<h4>Opciones</h4>
<ul>
  <li><b>Corregir el reloj</b>: horas sumadas a la fecha de la cámara y a la del nombre (no a la de modificación, ni a un nombre solo con el día), para una cámara con el reloj mal o fotos de viaje en otro huso horario. Vale para todos los archivos de la lista: para corregir solo los de una cámara, filtre la lista antes.</li>
  <li><b>Organizar en carpetas por fecha</b>: plantilla de carpeta con <code>/</code> entre los niveles (<code>{año}/{año}-{mes}</code>, <code>{año}/{mes} - {nombre del mes}</code>…). Las carpetas se crean en la carpeta que añadió; si ya es una carpeta de fecha (<code>Fotos/2019/2019-05</code>), en la carpeta de encima. Las carpetas que queden vacías se borran.</li>
  <li><b>Corregir la fecha del archivo</b>: la fecha de modificación de los archivos renombrados o movidos pasa a ser la de la foto, lo que ayuda a galerías y móviles que ordenan por ella. El contenido del archivo no cambia, y deshacer devuelve la fecha antigua.</li>
</ul>
<p>Las <b>copias idénticas</b> y las <b>fotos parecidas</b> siguen funcionando con la regla: aparecen en el pie, y un doble clic en la marca de una abre la comparación lado a lado, desde donde las que sobran van a la Papelera (vea <b>Copias idénticas</b>).</p>
<p><b>Limitaciones:</b></p>
<ul>
  <li>La primera vez, se lee el comienzo de cada foto y vídeo para hallar la fecha: en un disco duro externo, cerca de un minuto para 13 mil archivos, con el avance abajo (<b>Leyendo las fechas de las fotos…</b>). La lista aparece cuando termina la lectura; después, la vista previa es instantánea. La ventana sigue respondiendo, y la búsqueda de copias y de imágenes parecidas espera esta lectura. Copiar una colección grande a otra carpeta lleva el tiempo de copiar todos los datos.</li>
  <li>Algunos RAW pueden no tener la fecha leída (como CR3, RAF, ORF, RW2, X3F, CRW y MRW): toman la del JPEG con el mismo nombre, o la del nombre. Los vídeos MTS, M2TS y AVI solo tienen la fecha del nombre o la de modificación.</li>
  <li>La fecha de grabación de un vídeo viene, por la regla del formato, en hora universal (UTC), y se muestra en el huso de este ordenador; un vídeo grabado en otro huso queda con la hora de aquí. Los vídeos del iPhone dicen su huso y quedan bien. Algunas cámaras graban la hora local como si fuera UTC: corríjalo con <b>Corregir el reloj</b>.</li>
  <li>Dos archivos distintos con el mismo nombre y extensiones distintas en una carpeta (<code>foto.jpg</code> y <code>foto.png</code>) se toman como una sola foto.</li>
  <li>Con carpetas por fecha, una foto en una subcarpeta (<code>Fotos/Viaje</code>) sale de ella hacia la carpeta de la fecha.</li>
  <li><b>Corregir la fecha del archivo</b> vale solo para los archivos renombrados o movidos; uno que ya tiene el nombre correcto no se toca. La fecha de creación no cambia.</li>
  <li>No se graba nada dentro de las fotos, y el lugar donde se tomaron (GPS) no se usa.</li>
</ul>`,
  },
  {
    id: "preview",
    title: "Vista previa y conflictos",
    html: `
<p>La columna <b>Nuevo nombre</b> muestra el resultado de las reglas al instante. En las filas que cambian:</p>
<ul>
  <li>en <b>Nombre actual</b>, lo que se va aparece <span class="del">en rojo, tachado</span>;</li>
  <li>en <b>Nuevo nombre</b>, lo que entra aparece <span class="ins">en azul</span>;</li>
  <li>lo que solo cambió de lugar (el autor que pasó delante del título, por ejemplo) aparece <span class="mov">subrayado con puntos</span>, en los dos lados.</li>
  <li>un archivo que cambia de carpeta recibe un icono de carpeta en el nombre nuevo, y la columna <b>Carpeta</b> muestra, en azul, la carpeta completa adonde va (<code>→ ~/Música/Artista/Álbum (2019)</code>), escrita como la carpeta actual de los demás archivos.</li>
  <li>con una regla de búsqueda activa (película, serie, música, libro, cómic), un archivo que ella no trata (un <code>.zip</code> en la carpeta de la serie, por ejemplo) recibe la etiqueta gris <b>fuera de la regla</b>; pase el ratón para ver el motivo. La fila sigue normal: puede renombrarlo a mano, pero la búsqueda no lo tocará.</li>
</ul>
<p>En la primera columna, ● indica que el archivo se renombrará, un <span class="bad">!</span> rojo es un conflicto y un <span class="warn">!</span> amarillo es un aviso. Pase el ratón para ver el motivo.</p>
<h4>Editar a mano</h4>
<p>Haga doble clic en el nuevo nombre (o seleccione un archivo y pulse <kbd>F2</kbd>) para escribir el nombre que quiera. <kbd>Enter</kbd> confirma y <kbd>Esc</kbd> cancela. Un nombre editado a mano ignora las reglas, pero la carpeta a la que ellas llevan el archivo (la de la serie, la de la película) sigue valiendo; ↺ vuelve a usar las reglas.</p>
<h4>Carpetas antiguas que se quedan con archivos</h4>
<p>Cuando las reglas dan un nombre nuevo a la carpeta de una serie, una película, un álbum, un autor o una serie de cómics, la carpeta nueva se crea <b>junto a</b> la antigua y los archivos pasan a ella. La carpeta antigua que quede vacía se borra. Si todavía quedan archivos en ella (extras, imágenes, un archivo en conflicto, un archivo fuera de la lista), no se borra: pasa a llamarse como la nueva, con <code>(archivos adicionales)</code> al final, y las dos aparecen juntas. Por ejemplo, <code>Nombre de la Película.1999.1080p.BluRay</code> con un <code>sample.mkv</code> queda <code>Nombre de la Película (1999) [tmdbid-12345] (archivos adicionales)</code>, junto a <code>Nombre de la Película (1999) [tmdbid-12345]</code>.</p>
<ul>
  <li>Antes de renombrar, la barra de abajo muestra, en amarillo, <b>carpeta(s) con sobrantes</b> (pase el ratón para ver cuáles), y la confirmación dice qué carpeta y cuántos archivos quedan en ella, aunque la confirmación esté desactivada.</li>
  <li><b>Deshacer</b> devuelve los archivos y el nombre antiguo de la carpeta.</li>
  <li>La carpeta personal, Descargas, Documentos, Vídeos, Música, Imágenes, el Escritorio y la raíz de un disco nunca se renombran.</li>
  <li>Una carpeta que pasa a ser dos (<code>Título del Libro</code> → <code>Autor/Título del Libro</code>) se queda con su nombre: no hay una sola carpeta nueva junto a ella. Lo mismo vale para una carpeta cuyos archivos van a lugares distintos (una carpeta con canciones de varios artistas).</li>
  <li>Una película solo recibe su carpeta nueva al lado cuando la carpeta donde está lleva su nombre (con o sin marcas como 1080p) o solo tiene la película y lo que la acompaña: subtítulos, imágenes, <code>.nfo</code> y <code>.txt</code>. Una carpeta con otro vídeo, un documento o una hoja de cálculo no cuenta: la carpeta de la película se crea dentro.</li>
</ul>
<h4>Conflictos</h4>
<p>Los archivos con conflicto no se renombran. Los motivos son:</p>
<ul>
  <li>nombre vacío, o que contiene <code>/</code>;</li>
  <li>nombre de más de 255 bytes (en Windows, de más de 255 caracteres);</li>
  <li>dos archivos quedarían con el mismo nombre en la misma carpeta;</li>
  <li>ya existe un archivo con ese nombre fuera de la lista (nada se sobrescribe);</li>
  <li>en Windows: los caracteres <code>\\ : * ? " &lt; &gt; |</code>, o un punto o espacio al final del nombre.</li>
</ul>
<p>En otros sistemas, esos caracteres de Windows solo generan un aviso amarillo, que se puede desactivar en Opciones. En Windows, macOS y los discos que no distinguen mayúsculas (exFAT, FAT, NTFS, como los pendrives), los nombres que solo difieren en mayúsculas cuentan como iguales.</p>
<p><b>Exportar lista</b>, en el pie, guarda la lista tal como se ve (carpeta, nombre actual, nombre nuevo, carpeta a la que va, estado y problema) en un archivo CSV, que se abre en cualquier hoja de cálculo. Sirve para revisar con calma un lote grande antes de renombrar.</p>
<p>Cuando hay algo que falta (episodios, números, pistas, películas o volúmenes), el pie muestra el total, con una casilla al lado: <b>desmarcada</b>, las filas atenuadas desaparecen de la lista, para ver solo sus archivos; <b>marcada</b>, vuelven. No se busca nada de nuevo, y el total y la exportación siguen iguales. La elección queda guardada.</p>
`,
  },
  {
    id: "identical",
    title: "Copias idénticas",
    html: `
<p>Después de añadir archivos, el programa busca, en segundo plano, archivos de la lista con <b>contenido idéntico</b>, aunque tengan nombres distintos. No se borra ni se cambia nada: es solo un aviso.</p>
<ul>
  <li>Los archivos con copia llevan una etiqueta con un <b>número de grupo</b> junto al nombre: los archivos con el mismo número son idénticos entre sí, aunque tengan nombres distintos.</li>
  <li><b>Seleccione un archivo</b> y sus copias se resaltan en la lista. Pasar el ratón sobre la etiqueta muestra los nombres de las otras copias.</li>
  <li>El pie muestra cuántos archivos tienen copia. <b>Haga clic</b> para ver solo esos archivos, con las copias juntas; otro clic muestra todos. Desde ahí puede quitarlos de la lista o renombrarlos aparte.</li>
  <li>Cuando dos copias quedarían con el mismo nombre, el conflicto avisa que los archivos son idénticos.</li>
</ul>
<p>La búsqueda se hace por etapas para ser rápida: solo se comparan archivos del mismo tamaño; luego solo el inicio y el final; el archivo entero solo se lee cuando todo eso coincide. Con archivos grandes, el pie muestra el progreso. Los archivos vacíos, las carpetas y los enlaces simbólicos quedan fuera.</p>
<h4>Canciones repetidas</h4>
<p>Además de las copias idénticas, el programa encuentra la <b>misma canción en archivos distintos</b>: un MP3 y un FLAC de la misma pista, o una canción en su álbum y otra vez en un recopilatorio. Reciben una etiqueta con contorno, <b>♪</b> y el número del grupo (pase el ratón para ver cuáles son las otras), y el pie muestra <b>N canción(es) repetida(s)</b>: haga clic para ver solo esas, agrupadas. No se borra ni se cambia nada.</p>
<ul>
  <li>Cuentan como la misma canción: mismo <b>título</b> y mismo <b>artista</b> en las etiquetas, sin importar mayúsculas, acentos y puntuación, y sin lo que va entre paréntesis o corchetes (<code>(Remaster 2011)</code>); las colaboraciones (<code>feat.</code>) no cuentan. La <b>duración</b> debe ser la misma, con hasta 3 segundos de diferencia.</li>
  <li>Las copias idénticas de un mismo archivo ya aparecen como copias; solo entran aquí cuando hay también otro archivo de la misma canción.</li>
</ul>
<p><b>Limitaciones:</b> solo cuentan las etiquetas, no el sonido. Las canciones sin título, sin artista o sin duración quedan fuera; las etiquetas erróneas dan falsas alarmas o dejan pasar repetidas. Una versión en vivo o remezclada con el mismo título y casi la misma duración cuenta como la misma canción, y la misma grabación con títulos escritos de forma distinta (<code>Pt. 1</code> y <code>Part One</code>) no se reconoce. La primera vez se leen las etiquetas de cada canción de la lista, lo que tarda un poco en carpetas grandes.</p>
<h4>Imágenes parecidas</h4>
<p>Después, el programa compara las <b>imágenes</b> de la lista y encuentra las que se parecen: la misma foto en otro tamaño o formato, recomprimida o con pequeños retoques (brillo, un texto encima), y fotos tomadas una tras otra. Reciben una etiqueta con contorno, un icono de imagen y el número del grupo (pasa el ratón para ver cuáles son las otras), y el pie muestra <b>N imagen(es) parecida(s)</b>: haz clic para ver solo esas, agrupadas. No se borra ni se cambia nada.</p>
<ul>
  <li>Cada imagen se convierte en una "huella" de su dibujo, tomada de una copia muy pequeña en tonos de gris, ya girada como se muestra la foto. Las imágenes con huellas casi iguales quedan en el mismo grupo; una imagen parecida a otras dos junta las tres.</li>
  <li>Formatos: JPEG, PNG, WebP, GIF (el primer cuadro), BMP y TIFF. HEIC y archivos RAW de cámara solo entran cuando llevan dentro una miniatura en JPEG (casi todos los RAW la llevan; los HEIC casi nunca).</li>
  <li>Las copias idénticas de un mismo archivo ya aparecen como copias; solo entran aquí cuando hay también otra imagen parecida.</li>
  <li>Las huellas se guardan en la carpeta de caché del programa: la próxima vez solo se leen las imágenes nuevas o cambiadas. Una huella que no se usa en 90 días se olvida.</li>
</ul>
<p><b>Limitaciones:</b> solo cuenta la forma general de la imagen, no lo que muestra. Una foto <b>recortada</b>, girada a mano o reflejada no se reconoce, y dos imágenes distintas con la misma composición (capturas de pantalla del mismo programa, páginas de texto, cielos) pueden tomarse como parecidas. Las imágenes de un solo color quedan fuera. La primera vez se lee cada imagen entera: en un disco duro externo, unos miles de fotos tardan minutos (unos 2 minutos para 3.700 fotos), con el progreso en el pie.</p>
<h4>Comparar y mandar a la Papelera</h4>
<p>Con el filtro de <b>imágenes parecidas</b> o de <b>copias idénticas</b> activo, el pie muestra <b>Comparar</b>; hacer doble clic en la etiqueta de grupo de una fila también abre la comparación en ese grupo. La ventana muestra un grupo cada vez, lado a lado:</p>
<ul>
  <li>Cada archivo aparece con la imagen, el nombre, la carpeta, el tamaño en píxeles y en bytes, y la fecha en que se tomó la foto (o, sin ella, la de la última modificación). El de más píxeles (o, si empatan, el archivo mayor) recibe la marca <b>★ mayor</b>, y el más antiguo, <b>más antigua</b>.</li>
  <li>Haz clic en una imagen para verla grande; las flechas recorren las imágenes del grupo (viéndola grande) o los grupos. <b>Esc</b> vuelve o cierra.</li>
  <li>Marca <b>Mandar a la Papelera</b> en los archivos que no quieras, en tantos grupos como quieras. <b>Conservar solo la mayor</b> marca todos los del grupo menos el mayor.</li>
  <li>Nada se mueve hasta que hagas clic en <b>Mover a la Papelera</b> y confirmes. Si en algún grupo están todos marcados, la confirmación lo avisa. Los archivos van a la <b>Papelera del sistema</b> (de donde se pueden recuperar hasta que se vacíe) y salen de la lista; los grupos se rehacen.</li>
</ul>
<p><b>Limitaciones:</b> mandar a la Papelera no queda en el Historial: para recuperar un archivo, usa la Papelera del sistema (en macOS, arrástralo de la Papelera a una carpeta: el <b>Devolver</b> del Finder no aparece para él). Los archivos que no son imágenes (copias idénticas de vídeos, canciones) aparecen sin imagen, solo con sus datos. En un disco sin Papelera (algunos pendrives y discos de red), el archivo se queda donde está y el programa avisa.</p>`,
  },
  {
    id: "apply",
    title: "Renombrar y deshacer",
    html: `
<p><b>Renombrar</b> (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) aplica los nuevos nombres de los archivos visibles que cambian y no tienen conflicto. Antes, el programa pide confirmación (se puede desactivar en Opciones).</p>
<ul>
  <li>El renombrado se hace en dos pasos, pasando por un nombre temporal. Por eso se pueden intercambiar nombres (a ↔ b), y nunca se sobrescribe un archivo.</li>
  <li>Las carpetas y los archivos de dentro se pueden renombrar juntos: primero el contenido, después la carpeta.</li>
  <li>Las hojas de pistas y listas de reproducción (<code>.cue</code>, <code>.m3u</code>, <code>.m3u8</code>) que citan archivos renombrados o movidos se actualizan con los nuevos nombres. Un <code>.cue</code> que solo cita archivos inexistentes (el disco entero en un archivo que luego se dividió en pistas) se rehace con una pista por archivo, emparejando cada <code>TRACK</code> con la canción de la carpeta que tiene ese número en las etiquetas; si no es posible, se borra. Las listas <code>.m3u</code> en las que no existe ninguna canción también se borran. Son los únicos cambios que Renome hace en archivos; el contenido original queda guardado en el historial, y deshacer lo devuelve.</li>
  <li>Al terminar, un aviso en la esquina inferior derecha indica cuántos archivos se renombraron y desaparece solo.</li>
  <li>Si un archivo falla (sin permiso, borrado por otro programa…), los demás siguen, y al final aparece la lista de lo que falló y por qué.</li>
  <li>Después de renombrar, las reglas se desactivan (se puede cambiar en Opciones), para no aplicarlas dos veces.</li>
</ul>
<h4>Enviar a otra carpeta</h4>
<p>Junto al botón Renombrar, el selector dice adónde van los archivos: <b>En la misma carpeta</b> (lo normal: solo renombra), <b>Mover a otra carpeta…</b> o <b>Copiar a otra carpeta…</b>. Sirve para ordenar una carpeta de descargas directamente en su biblioteca, también en otro disco. La elección no se guarda y vuelve a "En la misma carpeta" después de cada envío: la lista pasa a mostrar los archivos en su lugar nuevo, y enviarlos otra vez los mandaría de nuevo.</p>
<ul>
  <li>Las carpetas que crean las reglas (artista y álbum, película, serie y temporada, autor, serie de cómics) quedan dentro de la carpeta elegida: <code>Destino/Artista/Álbum (1999)/01 - Canción.flac</code>.</li>
  <li>Los demás archivos mantienen sus subcarpetas a partir de la carpeta que usted agregó: con <code>Fotos</code> agregada, <code>Fotos/2024/Playa/x.jpg</code> va a <code>Destino/2024/Playa/x.jpg</code>. Los archivos agregados uno por uno van directamente al destino.</li>
  <li>Van todos los archivos visibles, incluso los que mantienen el nombre. La columna Carpeta muestra dónde quedará cada uno.</li>
  <li><b>Copiar</b>: los originales quedan exactamente como están, y las copias reciben los nombres nuevos. Portadas, <code>.nfo</code>, listas y etiquetas grabadas van a las copias. Las listas <code>.cue</code>/<code>.m3u</code> copiadas junto pasan a citar las copias; las originales no cambian. La fecha de modificación de cada archivo se mantiene en su copia.</li>
  <li>Las carpetas que queden vacías en el origen se borran, pero solo las que están dentro de la carpeta que usted agregó: ella misma, y las de fuera, quedan.</li>
  <li><b>Mover</b> a otro disco: el archivo se copia y solo se borra del disco de origen cuando la copia está completa y verificada.</li>
  <li>Durante la copia aparecen el progreso y el botón <b>Detener</b>: lo que ya se copió queda (y se puede deshacer), y el resto queda como estaba.</li>
  <li>Cada copia se hace con un nombre provisional y solo recibe el nombre final cuando está entera; si el programa se cierra a mitad, la copia a medias se borra al abrirlo de nuevo. Nada se sobrescribe: un nombre que ya existe en el destino aparece como conflicto.</li>
  <li><b>Deshacer</b> una copia borra las copias creadas, pero solo las que siguen como Renome las dejó. Una copia que usted editó después queda, y el aviso dice cuáles. Deshacer un movimiento trae los archivos de vuelta, incluso desde otro disco.</li>
</ul>
<p><b>Limitaciones</b>: no se comprueba antes el espacio libre en el destino; si se acaba, los archivos que no quepan fallan (sin dejar nada a medias) y aparecen en la lista de errores. Copiar muchos archivos grandes lleva el tiempo del disco: Renome no acelera la copia. Las carpetas agregadas como elementos se copian o mueven enteras.</p>
<h4>Historial</h4>
<p>Cada renombrado queda registrado en el <b>Historial</b>, incluso después de cerrar el programa. Allí puede:</p>
<ul>
  <li><b>Deshacer</b>: recupera los nombres anteriores.</li>
  <li><b>Reintentar</b>: aparece cuando parte del deshacer falló (por ejemplo, un archivo se movió después). Devuelva el archivo a su sitio e inténtelo de nuevo.</li>
  <li><b>✕</b>: borra la entrada del historial. Los archivos no cambian, pero ese renombrado ya no se podrá deshacer.</li>
  <li><b>Vaciar historial</b>: borra todas las entradas de una vez, con el mismo efecto.</li>
</ul>
<p class="tip"><kbd>Ctrl</kbd>+<kbd>Z</kbd> deshace cambios en la <i>lista</i> (añadir, quitar, mover, editar nombre). Para deshacer un renombrado ya hecho, use el Historial.</p>
<p>Si se cierra el programa o se apaga el equipo en medio de un renombrado (o de un deshacer), la próxima vez que Renome se abra termina con seguridad: los archivos que aún tenían un nombre temporal recuperan el nombre de antes, los que ya se habían renombrado entran en el <b>Historial</b>, para deshacerlos si quiere, y los que un deshacer ya había devuelto salen de él. No se pierde ningún archivo.</p>
`,
  },
  {
    id: "recipes",
    title: "Recetas",
    html: `
<h4>Fotos con la fecha en que se tomaron</h4>
<p>Insertar → Fecha en que se tomó la foto, formato <code>%Y-%m-%d %H.%M.%S </code>, al inicio.</p>
${ex("IMG_1234.jpg", "2024-07-15 14.32.10 IMG_1234.jpg")}
<h4>Numerar fotos de varias carpetas</h4>
<p>Añada las carpetas (con subcarpetas). Numerar → plantilla <code>{carpeta} {n}</code>, 3 dígitos, reiniciar en cada carpeta.</p>
${ex("Playa/IMG_0001.jpg", "Playa/Playa 001.jpg")}
<h4>Limpiar nombres de episodios</h4>
<p>Reemplazar texto <code>.</code> por un espacio (Nombre sin extensión), luego Quitar fragmento desde el texto <code> 720p</code> hasta el final.</p>
${ex("Serie.S01E02.720p.WEB.mkv", "Serie S01E02.mkv")}
<h4>Unificar extensiones</h4>
<p>Extensión → minúsculas. Añada Extensión → Definir <code>jpg</code> con el filtro <code>*.jpeg</code> para unificarlas.</p>`,
  },
  {
    id: "shortcuts",
    title: "Atajos de teclado",
    html: `
<table>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Enter</kbd></td><td>Renombrar (funciona incluso escribiendo en una regla)</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Z</kbd></td><td>Deshacer el último cambio en la lista</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>O</kbd></td><td>Añadir archivos</td></tr>
  <tr><td><kbd>F2</kbd></td><td>Editar el nuevo nombre del archivo seleccionado</td></tr>
  <tr><td><kbd>↑</kbd> <kbd>↓</kbd> <kbd>Inicio</kbd> <kbd>Fin</kbd></td><td>Moverse por la lista de archivos y seleccionar (llegue a ella con <kbd>Tab</kbd> o un clic); con <kbd>Mayús</kbd>, amplía la selección</td></tr>
  <tr><td><kbd>Supr</kbd></td><td>Quitar los seleccionados de la lista</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>A</kbd></td><td>Seleccionar todos</td></tr>
  <tr><td><kbd>F1</kbd></td><td>Abrir esta ayuda</td></tr>
  <tr><td><kbd>Esc</kbd></td><td>Cerrar ventana o menú</td></tr>
</table>
<p>En macOS, use <kbd>⌘</kbd> en lugar de <kbd>Ctrl</kbd>.</p>`,
  },
  {
    id: "options",
    title: "Opciones",
    html: `
<p>El engranaje abre las opciones:</p>
<ul>
  <li><b>Idioma</b> y <b>Tema</b> (claro, oscuro o del sistema).</li>
  <li><b>Recordar las reglas al cerrar el programa</b>.</li>
  <li><b>Incluir archivos ocultos al añadir carpetas</b>.</li>
  <li><b>Pedir confirmación antes de renombrar</b>.</li>
  <li><b>Desactivar las reglas después de renombrar</b>.</li>
  <li><b>Avisar sobre nombres que Windows no acepta</b>: caracteres como <code>:</code> y <code>?</code>, punto o espacio al final, y nombres reservados como <code>CON</code> y <code>NUL</code>. Útil para archivos que van a Windows. En un pendrive o disco formateado para Windows (FAT, exFAT), esos nombres no se aceptan de ninguna manera: allí son un error, con o sin esta opción.</li>
  <li><b>Búsquedas en línea: guardar las respuestas durante</b> 1 día, 7 días (lo predeterminado), 30 días, 6 meses, o no guardar. Las respuestas de TMDB, MusicBrainz, AcoustID, Discogs, Open Library, Grand Comics Database, Comic Vine, Metron, MangaDex, AniList y MangaUpdates se guardan en el disco; buscar de nuevo los mismos archivos es instantáneo y no gasta los límites de los servicios (el GCD sin cuenta acepta solo unas 30 consultas por hora). TMDB solo permite guardarlas hasta 6 meses, las de AniList se guardan 7 días como mucho (sus términos no permiten hacer una copia de sus datos) y las de Discogs valen solo 6 horas, sea cual sea la elección. Las claves y contraseñas no se guardan con ellas. <b>Vaciar</b> las borra todas y muestra cuánto ocupan. Limitación: una serie aún en emisión, una serie de cómics en curso o una corrección hecha en un catálogo solo aparecen cuando vence la respuesta guardada; para verlas ya, vacíe.</li>
  <li><b>TMDB</b>, <b>Google Books</b>, <b>Comic Vine</b>, <b>Metron</b>, <b>Grand Comics Database</b> y <b>Discogs</b>: las claves, los tokens y la cuenta que usan las reglas de película, serie, libro, cómic y música; cada uno tiene un botón <b>Probar</b>.</li>
  <li>Las claves, el token y la contraseña quedan solo en este equipo, <b>cifrados en el almacén de contraseñas del sistema</b> (el Llavero de GNOME o la Cartera de KDE en Linux, el Administrador de credenciales en Windows, el Llavero en macOS), no en el archivo de opciones. Limitación: un sistema sin almacén de contraseñas (algunas instalaciones mínimas de Linux) no tiene dónde cifrarlos; entonces quedan en el archivo de opciones, sin cifrar, y las Opciones lo avisan. El almacén los protege de quien lee los archivos del disco, no de otro programa abierto en su propia sesión.</li>
  <li>Cuando el almacén está bloqueado (por ejemplo, con inicio de sesión automático en Linux), el sistema pide su contraseña; el programa se abre igual, y las claves aparecen después de desbloquearlo. En macOS, como Renome no está firmado por Apple, el sistema puede preguntar si puede usar el Llavero, de nuevo con cada versión nueva; elija <b>Permitir siempre</b>. Si se niega el acceso (o el almacén no se desbloquea), las claves guardadas allí no se borran ni se reemplazan: las Opciones lo avisan y tienen un botón <b>Intentar de nuevo</b>.</li>
</ul>
<p>Las opciones se guardan en <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) o <code>~/Library/Application Support/renome</code> (macOS). El historial está en <code>~/.local/share/renome/history</code> en Linux y en la misma carpeta de las opciones en los demás sistemas. Las respuestas guardadas de las búsquedas están en <code>~/.cache/renome/answers</code> (Linux), <code>%LOCALAPPDATA%\\renome\\answers</code> (Windows) o <code>~/Library/Caches/renome/answers</code> (macOS).</p>`,
  },
];
