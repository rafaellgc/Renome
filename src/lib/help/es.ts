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
<p>Los archivos ocultos (que empiezan con punto, como <code>.config</code>) quedan fuera al añadir carpetas, salvo que active la opción correspondiente.</p>
<p>También puede abrir el programa con archivos: <code>renome foto1.jpg foto2.jpg carpeta/</code>.</p>

<h4>Filtrar</h4>
<p>El campo <b>Filtrar</b> muestra solo los archivos que coinciden. <b>Solo se renombran los archivos visibles</b>, y la numeración cuenta solo esos.</p>
<ul>
  <li>Términos separados por comas o espacios: <code>jpg, vacaciones</code> muestra los nombres que contienen "jpg" <i>o</i> "vacaciones".</li>
  <li><code>*</code> y <code>?</code> son comodines: <code>*.png</code> (termina en .png), <code>IMG_????.jpg</code> (cuatro caracteres cualesquiera).</li>
</ul>

<h4>Ordenar y organizar</h4>
<ul>
  <li>Haga clic en el título de una columna para ordenar; otra vez para invertir.</li>
  <li>El orden predeterminado es por <b>Nuevo nombre</b>: la lista se reordena mientras cambia las reglas.</li>
  <li>Seleccione con clic, <kbd>Ctrl</kbd>+clic (varios) y <kbd>Mayús</kbd>+clic (un intervalo).</li>
  <li><b>⤒ ⤓</b> mueven los seleccionados hacia arriba o abajo, útil para ajustar la numeración. No funcionan con el orden por Nuevo nombre.</li>
  <li><b>Quitar</b> (o <kbd>Supr</kbd>) saca los seleccionados de la lista; <b>Limpiar</b> la vacía. Los archivos del disco no se tocan.</li>
  <li><kbd>Ctrl</kbd>+<kbd>Z</kbd> deshace el último cambio en la lista.</li>
</ul>`,
  },
  {
    id: "rules",
    title: "Cómo funcionan las reglas",
    html: `
<p>Las reglas se aplican <b>de arriba abajo</b>, cada una sobre el resultado de la anterior. El orden importa: numerar y luego cambiar mayúsculas no da lo mismo que al revés.</p>
<ul>
  <li>La <b>casilla</b> del título activa y desactiva la regla sin borrarla.</li>
  <li><b>↑ ↓</b> cambian el orden; <b>✕</b> quita la regla; un clic en el título pliega la tarjeta.</li>
  <li>Una regla con error (una expresión inválida, por ejemplo) queda con borde rojo y se ignora hasta que se corrija.</li>
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
  <li><b>Cada Palabra, excepto de/la/y/of…</b>: → <code>Foto de la Playa</code>. Las palabras cortas de enlace (de, del, la, los, y, con, of, the, and, da, do…) quedan en minúsculas, salvo al inicio del nombre.</li>
</ul>
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
  <li><b>Acentos</b>: <code>Canción</code> → <code>Cancion</code></li>
  <li><b>Dígitos</b>: <code>foto123</code> → <code>foto</code></li>
  <li><b>Caracteres</b>: cada carácter escrito se quita donde aparezca. <code>_#</code> quita todos los <code>_</code> y <code>#</code>.</li>
</ul>`,
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
</ul>
<p>Esta regla no afecta a las carpetas.</p>`,
  },
  {
    id: "r-movie",
    title: "Película (TMDB)",
    sub: true,
    html: `
<p>Da a cada película su nombre oficial de <b>TMDB</b> (themoviedb.org), en el formato que reconoce Jellyfin. Necesita una <b>clave de la API de TMDB</b>, gratuita, introducida en Opciones.</p>
${ex("El.Senor.de.los.Anillos.2001.1080p.BluRay.x264.mkv", "El Señor de los Anillos - La comunidad del anillo (2001) [tmdbid-120].mkv")}
<ol>
  <li>Añada la regla y haga clic en <b>Buscar en TMDB</b>. El programa toma el título y el año del nombre de cada archivo (los puntos pasan a espacios; se quitan corchetes, paréntesis y marcas como 1080p, BluRay, x264) y busca en TMDB.</li>
  <li>Cuando título y año coinciden sin duda, la película se elige sola (<b>✓</b> junto al nuevo nombre). Si hay dudas (<b>?</b>), se abre la ventana de revisión: haga clic en la película correcta (o pulse 1–9). El año del archivo puede diferir en uno del de TMDB; se tiene en cuenta.</li>
  <li>Si ninguna sirve, corrija el título o el año en el campo de búsqueda de la ventana, o elija <b>Ninguna de estas</b> (<b>✕</b>: el archivo queda como está).</li>
</ol>
<ul>
  <li><b>Plantilla</b>: <code>{título}</code>, <code>{original}</code> (título original), <code>{año}</code> e <code>{id}</code>. La predeterminada es <code>{título} ({año}) [tmdbid-{id}]</code>.</li>
  <li><b>Cambiar ":" por</b>: Windows no acepta dos puntos; por defecto pasan a <code> - </code>. Las barras pasan a guiones y se quitan <code>? * &lt; &gt; |</code>.</li>
  <li>Los subtítulos y otros archivos con el mismo nombre que la película reciben la misma elección. Los subtítulos conservan el idioma al final del nombre: <code>Matrix (1999) [tmdbid-603].pt-BR.srt</code>.</li>
  <li><b>Crear una carpeta para cada película</b>: la película y sus subtítulos van a una carpeta con el nuevo nombre, como recomienda Jellyfin: <code>Películas/Matrix (1999) [tmdbid-603]/Matrix (1999) [tmdbid-603].mkv</code>. Una película que ya está en la carpeta correcta no se mueve de nuevo. Deshacer devuelve los archivos y borra la carpeta si queda vacía.</li>
  <li>Para quitar las tildes, añada la regla <b>Quitar caracteres → Acentos</b> después de esta.</li>
</ul>
<h4>Metadatos (.nfo y póster)</h4>
<p>Con <b>Guardar .nfo y póster junto a la película</b>, al renombrar se crean, junto a cada vídeo, <code>Nombre (año) [tmdbid-…].nfo</code> y el póster; opcionalmente también la imagen de fondo (fanart) y el logo de la película. Dentro de la carpeta de la película, las imágenes se llaman <code>poster.jpg</code>, <code>fanart.jpg</code> y <code>logo.png</code>; sueltas, llevan delante el nombre de la película (<code>…-poster.jpg</code>). Jellyfin, Kodi y Emby leen estos archivos.</p>
<p>El <code>.nfo</code> incluye todo lo que TMDB tiene y Jellyfin lee: título y título original, año y fecha de estreno, sinopsis y eslogan, duración, nota y votos, la clasificación por edades del país del idioma elegido (ej.: ES-12), géneros, palabras clave (como etiquetas), países, estudios, colección (ej.: "El Señor de los Anillos - Colección"), tráiler de YouTube, dirección, guion, reparto con personajes y fotos, productores, compositor y los ids de TMDB e IMDb. El archivo de la película no cambia, los archivos que ya existen no se reemplazan y <b>deshacer</b> el renombrado borra lo creado.</p>
<p>Las películas que <b>ya tienen el nombre correcto</b> también reciben los metadatos: el botón Renombrar las cuenta y la confirmación lo avisa. Si el nombre ya trae <code>[tmdbid-…]</code>, la película se reconoce directamente por ese id, sin buscar. Así una biblioteca ya organizada puede recibir solo los .nfo y las imágenes.</p>
<p class="tip">Solo se envían a TMDB el título y el año buscados, nunca el nombre completo del archivo ni las carpetas.</p>`,
  },
  {
    id: "r-series",
    title: "Serie (TMDB)",
    sub: true,
    html: `
<p>Da a cada episodio el nombre oficial de <b>TMDB</b>, con temporada, número y título, en el formato que Jellyfin reconoce. Usa la misma clave de la API que la regla de películas.</p>
${ex("Breaking.Bad.S01E03.720p.WEB-DL.mkv", "Breaking Bad (2008) - S01E03 - ...Y la bolsa está en el río.mkv")}
<ol>
  <li>En el campo <b>Serie</b>, escriba el nombre o el ID de TMDB, o pegue el enlace de la página de la serie (<code>themoviedb.org/tv/1396-…</code>). Vacío, el programa usa el nombre que los archivos tienen en común, o el de la carpeta.</li>
  <li>Haga clic en <b>Buscar episodios</b>. Si hay más de una serie parecida, elija la correcta. El programa descarga la lista completa de episodios y busca el de cada archivo visible.</li>
  <li>Los episodios encontrados con seguridad reciben <b>✓</b>. Los dudosos (<b>?</b>) y los no encontrados (<b>✕</b>) van a la ventana de revisión.</li>
</ol>
<h4>Cómo se encuentra cada episodio</h4>
<ul>
  <li><b>Por el número</b>, en varios formatos: <code>S01E02</code>, <code>s1e2</code>, <code>1x02</code>, <code>T1 E2</code>, <code>Temporada 1 Episodio 2</code>, <code>Ep 02</code> (la temporada viene de la carpeta, como <code>Season 2</code> o <code>Temporada 2</code>), numeración absoluta (<code>Naruto - 125</code>), <code>201</code> (temporada 2, episodio 1) y fecha de emisión (<code>2024.03.15</code>). Los archivos con dos episodios (<code>S01E01E02</code>, <code>S01E01-02</code>) quedan como <code>S01E01-E02</code>.</li>
  <li><b>Por el título</b>: el texto después del número se compara con los títulos de todos los episodios, en su idioma, en inglés y en el idioma original, sin tener en cuenta acentos, mayúsculas ni pequeños errores. Sin número, basta el título si está claro.</li>
  <li><b>Cuando el número y el título no coinciden</b> (el nombre dice S02E08, pero el título es el de S01E03), el archivo queda con <b>?</b>, y la revisión muestra las dos opciones, sugiriendo la del título.</li>
</ul>
<h4>Revisión</h4>
<p>Muestra lo que se leyó en el nombre, las sugerencias (clic, 1–9, o <b>Enter</b> para la primera) y todos los episodios de la serie por temporada, con un filtro por título o número (<code>s01e03</code>). <b>Ctrl+clic</b> marca más de un episodio de la misma temporada, para un archivo con varios episodios. Los subtítulos leídos igual reciben la misma elección.</p>
<ul>
  <li><b>Plantilla</b>: <code>{serie}</code>, <code>{original}</code> (nombre original), <code>{año}</code> (estreno), <code>{temporada}</code>, <code>{episodio}</code> (dos dígitos), <code>{título}</code> e <code>{id}</code>. La predeterminada es <code>{serie} ({año}) - S{temporada}E{episodio} - {título}</code>.</li>
  <li>Los episodios sin traducción reciben el título en inglés (o en el idioma original), en lugar de "Episodio 4".</li>
  <li>Los subtítulos conservan el idioma al final del nombre: <code>….pt-BR.srt</code>.</li>
  <li>Los archivos que no están en la serie elegida quedan como están. Para varias series, filtre la lista y busque una por vez: cada archivo guarda la suya.</li>
</ul>
<h4>Carpetas de la serie y la temporada</h4>
<p>Con <b>Organizar en carpetas de la serie y la temporada</b>, cada episodio va a la estructura que recomienda Jellyfin: <code>Series/Breaking Bad (2008) [tmdbid-1396]/Season 01/…</code>. La carpeta de la serie se crea junto a la carpeta donde ya está la serie (si los archivos están en <code>Breaking Bad/Season 1</code>, la nueva carpeta queda junto a <code>Breaking Bad</code>, no dentro); con archivos sueltos, dentro de su carpeta. El nombre de la carpeta de la serie tiene su propia plantilla, con las mismas variables (predeterminada <code>{serie} ({año}) [tmdbid-{id}]</code>; use <code>{original}</code> para el nombre original). Las carpetas antiguas que quedan vacías se borran; las que aún tienen otros archivos se quedan. <b>Deshacer</b> lo devuelve todo a su sitio y vuelve a crear las carpetas antiguas.</p>
<h4>Metadatos (.nfo e imágenes)</h4>
<p>Con <b>Guardar .nfo e imágenes para Jellyfin</b>, al renombrar el programa crea:</p>
<ul>
  <li>en la <b>carpeta de la serie</b>: <code>tvshow.nfo</code> (título, título original, año, sinopsis, estado, duración, nota, clasificación por edad, géneros, palabras clave, cadenas y estudios, creadores, tráiler, reparto con personajes y los ID de TMDB, IMDb y TVDB), <code>poster.jpg</code> y el póster de cada temporada (<code>season01-poster.jpg</code>); opcionalmente <code>fanart.jpg</code> y <code>logo.png</code>;</li>
  <li>en la <b>carpeta de la temporada</b>: <code>season.nfo</code>, con el nombre y la sinopsis de la temporada;</li>
  <li>junto a <b>cada episodio</b>: <code>….nfo</code> (título, temporada y número, fecha de emisión, sinopsis, duración, nota, dirección, guion y actores invitados) y la miniatura <code>…-thumb.jpg</code>.</li>
</ul>
<p>Los archivos de la serie y la temporada solo se crean cuando los episodios están en una carpeta de la serie (con su nombre o con <code>[tmdbid-…]</code>), la de antes o la que crea la opción de carpetas. Los episodios que ya tienen el nombre correcto también reciben los metadatos. Los archivos que ya existen no se reemplazan, y deshacer borra lo creado.</p>
<p class="tip">Solo se envía a TMDB el nombre de la serie, nunca los nombres de los archivos ni las carpetas.</p>`,
  },
  {
    id: "preview",
    title: "Vista previa y conflictos",
    html: `
<p>La columna <b>Nuevo nombre</b> muestra el resultado de las reglas al instante. En las filas que cambian:</p>
<ul>
  <li>en <b>Nombre actual</b>, lo que se va aparece <span class="del">en rojo, tachado</span>;</li>
  <li>en <b>Nuevo nombre</b>, lo que entra aparece <span class="ins">en azul</span>.</li>
</ul>
<p>En la primera columna, ● indica que el archivo se renombrará, un <span class="bad">!</span> rojo es un conflicto y un <span class="warn">!</span> amarillo es un aviso. Pase el ratón para ver el motivo.</p>
<h4>Editar a mano</h4>
<p>Haga doble clic en el nuevo nombre (o seleccione un archivo y pulse <kbd>F2</kbd>) para escribir el nombre que quiera. <kbd>Enter</kbd> confirma y <kbd>Esc</kbd> cancela. Un nombre editado a mano ignora las reglas; ↺ vuelve a usar las reglas.</p>
<h4>Conflictos</h4>
<p>Los archivos con conflicto no se renombran. Los motivos son:</p>
<ul>
  <li>nombre vacío, o que contiene <code>/</code>;</li>
  <li>nombre de más de 255 bytes;</li>
  <li>dos archivos quedarían con el mismo nombre en la misma carpeta;</li>
  <li>ya existe un archivo con ese nombre fuera de la lista (nada se sobrescribe);</li>
  <li>en Windows: los caracteres <code>\\ : * ? " &lt; &gt; |</code>, o un punto o espacio al final del nombre.</li>
</ul>
<p>En otros sistemas, esos caracteres de Windows solo generan un aviso amarillo, que se puede desactivar en Opciones. En Windows y macOS, los nombres que solo difieren en mayúsculas cuentan como iguales.</p>`,
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
<p>La búsqueda se hace por etapas para ser rápida: solo se comparan archivos del mismo tamaño; luego solo el inicio y el final; el archivo entero solo se lee cuando todo eso coincide. Con archivos grandes, el pie muestra el progreso. Los archivos vacíos, las carpetas y los enlaces simbólicos quedan fuera.</p>`,
  },
  {
    id: "apply",
    title: "Renombrar y deshacer",
    html: `
<p><b>Renombrar</b> (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) aplica los nuevos nombres de los archivos visibles que cambian y no tienen conflicto. Antes, el programa pide confirmación (se puede desactivar en Opciones).</p>
<ul>
  <li>El renombrado se hace en dos pasos, pasando por un nombre temporal. Por eso se pueden intercambiar nombres (a ↔ b), y nunca se sobrescribe un archivo.</li>
  <li>Las carpetas y los archivos de dentro se pueden renombrar juntos: primero el contenido, después la carpeta.</li>
  <li>Si un archivo falla (sin permiso, borrado por otro programa…), los demás siguen, y al final aparece la lista de lo que falló y por qué.</li>
  <li>Después de renombrar, las reglas se desactivan (se puede cambiar en Opciones), para no aplicarlas dos veces.</li>
</ul>
<h4>Historial</h4>
<p>Cada renombrado queda registrado en el <b>Historial</b>, incluso después de cerrar el programa. Allí puede:</p>
<ul>
  <li><b>Deshacer</b>: recupera los nombres anteriores.</li>
  <li><b>Reintentar</b>: aparece cuando parte del deshacer falló (por ejemplo, un archivo se movió después). Devuelva el archivo a su sitio e inténtelo de nuevo.</li>
  <li><b>✕</b>: borra la entrada del historial. Los archivos no cambian, pero ese renombrado ya no se podrá deshacer.</li>
</ul>
<p class="tip"><kbd>Ctrl</kbd>+<kbd>Z</kbd> deshace cambios en la <i>lista</i> (añadir, quitar, mover, editar nombre). Para deshacer un renombrado ya hecho, use el Historial.</p>`,
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
  <li><b>Avisar sobre caracteres que Windows no acepta</b>: útil para archivos que van a un pendrive o a Windows.</li>
</ul>
<p>Las opciones se guardan en <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) o <code>~/Library/Application Support/renome</code> (macOS). El historial está en <code>~/.local/share/renome/history</code> en Linux y en la misma carpeta de las opciones en los demás sistemas.</p>`,
  },
];
