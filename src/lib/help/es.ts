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
  <li>La <b>casilla</b> del título activa y desactiva la regla sin borrarla. Desactivada, queda bloqueada: para cambiar sus opciones o buscar, actívela antes.</li>
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
  <li><b>Cambiar ":" por</b>: Windows no acepta dos puntos; por defecto pasan a <code> - </code>. Las barras pasan a guiones y se quitan <code>? * &lt; &gt; |</code>.</li>
  <li>Solo se buscan y renombran vídeos, subtítulos y carpetas; las imágenes, los <code>.nfo</code> y otros archivos de la lista quedan como están. Los subtítulos con el mismo nombre que la película reciben la misma elección. Los subtítulos conservan el idioma al final del nombre: <code>Nombre de la Película (1999) [tmdbid-12345].pt-BR.srt</code>.</li>
  <li><b>Crear una carpeta para cada película</b>: la película y sus subtítulos van a una carpeta con el nuevo nombre, como recomienda Jellyfin: <code>Películas/Nombre de la Película (1999) [tmdbid-12345]/Nombre de la Película (1999) [tmdbid-12345].mkv</code>. Una película que ya está en la carpeta correcta no se mueve de nuevo. Deshacer devuelve los archivos y borra la carpeta si queda vacía.</li>
  <li>Para quitar las tildes, añada la regla <b>Quitar caracteres → Acentos</b> después de esta.</li>
</ul>
<h4>Metadatos (.nfo y póster)</h4>
<p>Con <b>Guardar .nfo y póster junto a la película</b>, al renombrar se crean, junto a cada vídeo, <code>Nombre (año) [tmdbid-…].nfo</code> y el póster; opcionalmente también la imagen de fondo (fanart) y el logo de la película. Dentro de la carpeta de la película, las imágenes se llaman <code>poster.jpg</code>, <code>fanart.jpg</code> y <code>logo.png</code>; sueltas, llevan delante el nombre de la película (<code>…-poster.jpg</code>). Jellyfin, Kodi y Emby leen estos archivos.</p>
<p>El <code>.nfo</code> incluye todo lo que TMDB tiene y Jellyfin lee: título y título original, año y fecha de estreno, sinopsis y eslogan, duración, nota y votos, la clasificación por edades del país del idioma elegido (ej.: ES-12), géneros, palabras clave (como etiquetas), países, estudios, colección (ej.: "Nombre de la Colección"), tráiler de YouTube, dirección, guion, reparto con personajes y fotos, productores, compositor y los ids de TMDB e IMDb. El archivo de la película no cambia, los archivos que ya existen no se reemplazan y <b>deshacer</b> el renombrado borra lo creado.</p>
<p>Las películas que <b>ya tienen el nombre correcto</b> también reciben los metadatos: el botón Renombrar las cuenta y la confirmación lo avisa. Si el nombre ya trae <code>[tmdbid-…]</code>, la película se reconoce directamente por ese id, sin buscar. Así una biblioteca ya organizada puede recibir solo los .nfo y las imágenes.</p>
<h4>Limitaciones</h4>
<ul>
  <li>La película se busca por el título y el año del nombre del archivo. Los nombres muy abreviados, solo con siglas o con el título en otro idioma pueden no encontrarse; en ese caso, corrija el título en la revisión.</li>
  <li>Las películas con el mismo nombre (remakes, por ejemplo) solo se distinguen por el año; sin el año en el nombre, la elección suele ir a la revisión.</li>
  <li>Las películas raras, regionales o muy recientes pueden no estar en TMDB, o estar sin traducción, sinopsis o póster.</li>
</ul>
<p class="tip">Solo se envían a TMDB el título y el año buscados, nunca el nombre completo del archivo ni las carpetas.</p>`,
  },
  {
    id: "r-series",
    title: "Serie (TMDB)",
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
  <li>Los episodios sin traducción reciben el título en inglés (o en el idioma original), en lugar de "Episodio 4".</li>
  <li>Los subtítulos conservan el idioma al final del nombre: <code>….pt-BR.srt</code>.</li>
  <li>Los archivos que no están en la serie elegida quedan como están. Cada archivo guarda su serie; buscar de nuevo no toca los ya resueltos.</li>
</ul>
<h4>Carpetas de la serie y la temporada</h4>
<p>Con <b>Organizar en carpetas de la serie y la temporada</b>, cada episodio va a la estructura que recomienda Jellyfin: <code>Series/Nombre de la Serie (2008) [tmdbid-12345]/Season 01/…</code>. La carpeta de la serie se crea junto a la carpeta donde ya está la serie (si los archivos están en <code>Nombre de la Serie/Season 1</code>, la nueva carpeta queda junto a <code>Nombre de la Serie</code>, no dentro); con archivos sueltos, dentro de su carpeta. El nombre de la carpeta de la serie tiene su propia plantilla, con las mismas variables (predeterminada <code>{serie} ({año}) [tmdbid-{id}]</code>; use <code>{original}</code> para el nombre original). Las carpetas antiguas que quedan vacías se borran; las que aún tienen otros archivos se quedan. Por eso, después de renombrar, la carpeta antigua puede desaparecer (<code>Dibujos/Nombre de la Serie</code>) y los archivos aparecen en la nueva, junto a ella (<code>Dibujos/Nombre de la Serie (2008) [tmdbid-12345]/Season 01/…</code>): <b>no se borró nada, solo se movió</b>. <b>Deshacer</b> lo devuelve todo a su sitio y vuelve a crear las carpetas antiguas.</p>
<h4>Metadatos (.nfo e imágenes)</h4>
<p>Con <b>Guardar .nfo e imágenes para Jellyfin</b>, al renombrar el programa crea:</p>
<ul>
  <li>en la <b>carpeta de la serie</b>: <code>tvshow.nfo</code> (título, título original, año, sinopsis, estado, duración, nota, clasificación por edad, géneros, palabras clave, cadenas y estudios, creadores, tráiler, reparto con personajes y los ID de TMDB, IMDb y TVDB), <code>poster.jpg</code> y el póster de cada temporada (<code>season01-poster.jpg</code>); opcionalmente <code>fanart.jpg</code> y <code>logo.png</code>;</li>
  <li>en la <b>carpeta de la temporada</b>: <code>season.nfo</code>, con el nombre y la sinopsis de la temporada;</li>
  <li>junto a <b>cada episodio</b>: <code>….nfo</code> (título, temporada y número, fecha de emisión, sinopsis, duración, nota, dirección, guion y actores invitados) y la miniatura <code>…-thumb.jpg</code>.</li>
</ul>
<p>Los archivos de la serie y la temporada solo se crean cuando los episodios están en una carpeta de la serie (con su nombre o con <code>[tmdbid-…]</code>), la de antes o la que crea la opción de carpetas. Los episodios que ya tienen el nombre correcto también reciben los metadatos. Los archivos que ya existen no se reemplazan, y deshacer borra lo creado.</p>
<h4>Episodios que faltan</h4>
<p>Con <b>Mostrar los episodios que faltan</b> (activada por defecto), tras la búsqueda la lista muestra filas atenuadas para los episodios ya emitidos que ningún vídeo de la lista tiene, por ejemplo <code>S01E16 · Título</code>, con la fecha de emisión. Quedan en el lugar del episodio, entre sus vecinos, y la barra inferior dice cuántos faltan. Son solo un aviso: no se pueden seleccionar y no cuentan para totales, renombrado ni exportación.</p>
<p>Solo se revisan las temporadas de las que la lista tiene al menos un vídeo; las temporadas enteras que no tienes no aparecen. Tampoco los episodios aún no emitidos o sin fecha. Un episodio que solo tiene el subtítulo cuenta como faltante.</p>
<h4>Limitaciones</h4>
<ul>
  <li>Una serie solo se reconoce en el nombre del archivo por los nombres que tiene en TMDB (en cualquier idioma o país) o por lo que usted escribió. Un apodo o una sigla que TMDB no conoce hace que el archivo cuente como de otra serie: escriba la serie, o use ese nombre en el campo.</li>
  <li>La división en temporadas es la de TMDB. El anime y las series con numeración corrida a veces están divididos de otra forma allí, y los números del archivo no coinciden; revíselo en la revisión.</li>
  <li>Los especiales (temporada 0) suelen tener una numeración distinta de la de su emisión y casi siempre necesitan revisión.</li>
  <li>Un archivo con varios episodios solo se reconoce cuando son de la misma temporada.</li>
  <li>Los episodios que faltan siguen la numeración de TMDB: si tus archivos usan otra (la del DVD, o la numeración corrida de los animes), la lista de lo que falta puede salir mal. Los episodios ocultos por el filtro también cuentan como faltantes.</li>
</ul>
<p class="tip">Solo se envía a TMDB el nombre de la serie, nunca los nombres de los archivos ni las carpetas.</p>`,
  },
  {
    id: "r-music",
    title: "Música (etiquetas)",
    sub: true,
    html: `
<p>Da a cada canción el nombre que viene de las <b>etiquetas</b> guardadas en el archivo (ID3 del MP3, Vorbis del FLAC y del OGG, MP4 del M4A y otras), sin necesidad de internet.</p>
${ex("01.mp3", "01 - Título de la Canción.mp3")}
<ul>
  <li><b>Plantilla</b>: <code>{pista}</code>, <code>{título}</code>, <code>{artista}</code>, <code>{artista del álbum}</code>, <code>{álbum}</code>, <code>{año}</code>, <code>{disco}</code>, <code>{género}</code> y <code>{compositor}</code>. La predeterminada es <code>{pista} - {título}</code>. La pista tiene dos dígitos (tres en álbumes con más de 99 pistas).</li>
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
<p>Con <b>Organizar en carpetas de artista y álbum</b>, cada canción va a <code>{artista del álbum}/{álbum} ({año})</code> (la plantilla de carpetas se puede cambiar; cada <code>/</code> es un nivel). Así, <code>Música/Artista/Album/CD1/01.mp3</code> queda <code>Música/Artista/Álbum (1975)/01 - Título de la Canción.mp3</code>: las carpetas que ya llevan el nombre del artista, del álbum o de un disco (o que están dentro de la del artista) se reemplazan, no reciben carpetas nuevas dentro. Los archivos sueltos reciben las carpetas dentro de la carpeta donde están. Las pequeñas diferencias de escritura entre la carpeta y la etiqueta no molestan. Una carpeta que ya reúne álbumes con su nombre (<code>Programa (1000 - ..)</code> para los álbumes "Programa 1037 - …") se trata como colección: los álbumes quedan directamente en ella, sin carpetas de artista en medio.</p>
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
  <li><b>Incluir la portada en los archivos</b>: quita las imágenes que ya están dentro de cada canción y pone la portada del álbum (del Cover Art Archive o, si no hay, la <code>cover.jpg</code> de la carpeta).</li>
  <li>La grabación se hace en una copia, que solo ocupa el lugar del archivo al final; un corte a mitad no estropea la canción. Deshacer devuelve las etiquetas y las imágenes antiguas (en MP3 y M4A, los campos raros que quitó la limpieza y las etiquetas ID3v1 y APE no vuelven). La fecha de modificación cambia, para que los reproductores relean las etiquetas. Como el contenido cambia, un torrent ya no reconoce esos archivos.</li>
  <li>Las consultas se espacian (una cada 2 segundos), como pide MusicBrainz; cada álbum tarda unos 4 segundos.</li>
</ul>
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
<h4>Limitaciones</h4>
<ul>
  <li>Sin la búsqueda en MusicBrainz, el nombre depende solo de las etiquetas: etiquetas erróneas dan nombres erróneos.</li>
  <li>Los lanzamientos regionales, independientes, piratas o muy recientes pueden no estar en MusicBrainz; entonces valen las etiquetas.</li>
  <li>Por el sonido, grabaciones muy parecidas (una remasterización, una regrabación del propio artista) pueden confundirse. Cuando el álbum queda en duda, va a la revisión.</li>
  <li>Las canciones en Opus, WMA y otros formatos no citados arriba no se escuchan.</li>
  <li>En MP3 y M4A, deshacer la grabación de etiquetas no devuelve los campos raros ni las etiquetas ID3v1 y APE.</li>
</ul>
<p class="tip">Sin <b>Grabar las etiquetas en los archivos</b>, Renome solo lee las etiquetas; las canciones en sí no cambian. A MusicBrainz solo se envían el artista y el nombre del álbum.</p>`,
  },
  {
    id: "r-book",
    title: "Libro (Open Library)",
    sub: true,
    html: `
<p>Da a cada libro el nombre que viene de <b>Open Library</b> (openlibrary.org), un catálogo abierto de libros mantenido por el Internet Archive. Cuando Open Library no conoce el ISBN, se busca en <b>BrasilAPI</b> (brasilapi.com.br), que reúne los registros de las editoriales brasileñas en la CBL. Ninguno de los dos necesita clave.</p>
${ex("titulo_del_libro_autor.pdf", "Nombre del Autor - Título del Libro (1937).pdf")}
<ol>
  <li>Añada la regla y haga clic en <b>Buscar en Open Library</b>. Para cada libro (EPUB, MOBI, AZW3, PDF, FB2 y DjVu), el programa lee los datos grabados en el propio archivo: título, autor, ISBN, editorial y serie. Lo que falte viene del nombre del archivo.</li>
  <li>Con el <b>ISBN</b>, el libro se encuentra directamente, en su edición exacta (<b>✓</b>), en Open Library o en BrasilAPI. Sin él, el programa busca por el título y el autor: si coinciden sin dudas, el libro se elige solo; si no (<b>?</b>), se abre la revisión, con portada, autores, título original y número de ediciones, para que usted elija (haga clic o pulse 1–9).</li>
  <li>Si ninguno sirve, corrija el título o el autor en la ventana y busque de nuevo. También se puede buscar solo por el autor, con el título vacío, y ver sus libros. O marque <b>Ninguno de estos</b> (<b>✕</b>: el archivo queda como está).</li>
</ol>
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
  <li><b>Intercambiar título y autor</b>: el botón ⇄ de la revisión intercambia los dos campos, para nombres leídos al revés.</li>
  <li>El <b>título</b> es el de la edición en el idioma elegido en Opciones (<b>Idioma de los títulos</b>). Cuando el archivo trae otro título, de otra traducción, queda el del archivo. El título original aparece en la revisión y en <code>{título original}</code>.</li>
  <li>El mismo libro en varios formatos (<code>.epub</code>, <code>.pdf</code>, <code>.mobi</code>) con los mismos datos hace una sola búsqueda y recibe la misma elección.</li>
</ul>
<h4>Carpetas, portada y metadata.opf</h4>
<ul>
  <li><b>Organizar en carpetas de autor y libro</b>: cada libro va a <code>{autor}/{título}</code> (la plantilla de las carpetas se puede cambiar; cada <code>/</code> es un nivel), como espera Audiobookshelf. En bibliotecas grandes, <code>{inicial}/{autor invertido}/{título}</code> da <code>A/Apellido, Nombre/Título del Libro</code>. Un punto al final del nombre de una carpeta (<i>Apellido, C. S.</i>) se quita, porque Windows no acepta carpetas que terminen en punto. Las carpetas que ya tienen el nombre del autor o del libro se aprovechan, sin crear carpetas nuevas dentro. Las carpetas que queden vacías se borran; deshacer lo devuelve todo a su sitio.</li>
  <li><b>Guardar la portada</b>: guarda la portada de Open Library como <code>cover.jpg</code> en la carpeta del libro; sin la opción de carpetas, con el nombre del archivo del libro (<code>….jpg</code>).</li>
  <li><b>Guardar los datos del libro</b>: guarda <code>metadata.opf</code> (o <code>….opf</code>, sin la opción de carpetas), en formato OPF, con título, autores, sinopsis, temas, editorial, año, idioma, ISBN y serie. Audiobookshelf y los gestores de libros electrónicos leen este archivo.</li>
  <li>Los libros que ya tienen el nombre correcto también reciben la portada y los datos (cuentan en el botón <b>Renombrar</b>). Los archivos que ya existen no se reemplazan, y deshacer borra lo creado. El archivo del libro nunca cambia.</li>
</ul>
<h4>Limitaciones</h4>
<ul>
  <li><b>PDF</b>: muchos PDF tienen los datos del documento vacíos o erróneos (el nombre del archivo de Word, el nombre del equipo de quien lo hizo). Un PDF <b>escaneado</b> es solo una serie de imágenes, sin texto donde buscar el ISBN. En esos casos queda el nombre del archivo: un nombre claro, como <code>Autor - Título</code>, ayuda mucho; uno confuso lleva a la revisión. Conviene arreglar el nombre de esos archivos con las otras reglas, puestas encima de esta en la lista: la búsqueda lee el nombre tal como sale de ellas.</li>
  <li><b>Sin ISBN</b>, el libro se encuentra por el título y el autor, pero no se sabe qué edición es: el nombre usa la editorial y el ISBN del propio archivo, si los tiene, y <code>{año de edición}</code> queda vacío.</li>
  <li><b>Open Library</b> lo mantienen voluntarios. Los libros recientes, de editoriales pequeñas o de algunos países pueden no estar, o estar sin portada, sin año o repetidos, con datos algo distintos en cada copia. Faltan muchas traducciones: cuando el título no aparece, la revisión muestra los otros libros del autor, donde la obra puede estar con el título original, o no estar. En ese caso, marque <b>Ninguno de estos</b> y renómbrelo a mano.</li>
  <li><b>BrasilAPI</b> solo ayuda con libros con ISBN, casi siempre de editoriales brasileñas. Suele venir sin portada y sin año. El año de la primera publicación y la portada vienen de Open Library cuando tiene la obra; si no, <code>{año}</code> queda vacío y no hay portada. Entre los autores pueden venir el traductor o el ilustrador: cuando el archivo dice quién es el autor, solo queda él.</li>
  <li>A veces el <b>ISBN guardado en el archivo es de otro libro</b> (el de un anuncio al final, o el de la caja de una colección), o el catálogo tiene un error. Cuando el título y el autor encontrados no coinciden con los del archivo, el libro no se elige solo: va a la revisión.</li>
  <li><code>{año}</code> es el de la primera publicación de la obra: un libro publicado en 1937 queda con 1937, aunque la edición sea de 2019. Cuando el catálogo da un año posterior al de la edición guardada en el archivo, lo que es un error suyo, <code>{año}</code> queda vacío. El año de su edición (<code>{año de edición}</code>) solo se conoce cuando el libro se encuentra por el ISBN.</li>
  <li>La <b>serie</b> y el <b>volumen</b> vienen solo de los datos del archivo (de EPUB o de FB2) o del número en el nombre; Open Library casi nunca los tiene.</li>
  <li>Los cómics (CBZ, CBR) y las revistas no los trata esta regla.</li>
</ul>
<p class="tip">A Open Library solo se envían el título, el autor y el ISBN buscados, y a BrasilAPI solo el ISBN, nunca el nombre completo del archivo, las carpetas ni el contenido del libro.</p>`,
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
  <li>En <b>CBZ, CB7 y CBT</b>, el programa lee el <code>ComicInfo.xml</code> de dentro del archivo, cuando existe: serie, número, volumen, título, año, editorial, guion y dibujo. Es el dato más fiable, con una excepción: si la serie del <code>ComicInfo.xml</code> es otra que la del nombre del archivo (el original en inglés de una edición traducida, por ejemplo), valen la serie y el título del nombre, que es como usted llamó al archivo.</li>
  <li>Sin él (y siempre en <b>CBR, PDF y EPUB</b>), usa el nombre del archivo. Entiende el número (<code>Serie 012</code>, <code>#12</code>, <code>nº 12</code>, <code>Número 12</code>, <code>#12.1</code>), el volumen o tomo (<code>v02</code>, <code>Vol. 3</code>, <code>Vol. III</code>, <code>(Vol.03)</code>, <code>Tomo 5</code>, <code>T05</code>, <code>Libro 3</code>), el capítulo de manga (<code>Cap. 21</code>, <code>Ch. 21</code>, <code>c045</code>), el año entre paréntesis y el total (<code>(de 6)</code>, <code>#2 de 6</code>). Lo que hay entre el volumen y el número es de la serie (<code>Serie Vol. 2 Anual #01</code> es el anual, no el número 1). Un número seguido de una palabra es parte del nombre (<code>Nombre con 714 Palabras</code>, <code>10 Años</code>), y un número al principio seguido de un guion es el del número (<code>19 - Nombre de la Historia</code>). Las marcas como <code>(Digital)</code>, <code>(c2c)</code> y los nombres de grupos entre corchetes o paréntesis se ignoran.</li>
  <li>Un archivo solo con el número (<code>012.cbz</code>) toma la serie de su carpeta; una carpeta <code>Nombre de la Serie (2012)</code> también da el año de la serie, y una carpeta <code>Vol. 3</code>, el volumen.</li>
  <li>En un nombre que empieza por su número (<code>19 - Nombre de la Historia</code>), el número es el del principio: los números dentro del título no cuentan.</li>
  <li><b>La serie es el nombre de la carpeta</b>: para una carpeta de una sola serie, con archivos que traen el número y el título de la historia (<code>Nombre de la Serie (Completo)/01 - Nombre de la Historia</code>). Cada archivo queda con la serie de la carpeta (sin el año y lo que está entre paréntesis), y lo que traía su nombre pasa a ser el título: <code>Nombre de la Serie #001 - Nombre de la Historia</code>. Viene desactivada, porque en una carpeta que no es de una serie (<code>Descargas</code>) su nombre pasaría a ser la serie de todos.</li>
  <li>Un número único o recopilatorio sin numeración queda con la serie y el año (<code>Nombre del Álbum (2024)</code>), sin las marcas del nombre original.</li>
  <li>Algunos programas guardan las letras acentuadas como su código entre guiones bajos (<code>M_244_nica</code> por <i>Mônica</i>); el programa las restituye.</li>
  <li>Los archivos en los que no se encuentra ni la serie quedan como están.</li>
</ul>
<h4>Buscando en un catálogo</h4>
<p>En <b>Dónde buscar</b>, elija el catálogo:</p>
<ul>
  <li><b>Grand Comics Database</b> (comics.org): hecho por voluntarios, con ediciones de muchos países. Da el nombre de la serie como se publicó, el año, la editorial y la numeración. Pide el <b>acceso de una cuenta gratuita</b>: se pide al buscar y queda <b>solo en la memoria</b> hasta cerrar el programa; nunca se guarda. Sin cuenta, el GCD acepta solo unas 30 consultas por hora, y puede dejar de aceptarlas.</li>
  <li><b>MangaDex</b> (mangadex.org): manga, sin cuenta ni clave. Da el nombre y el año de la serie (en su idioma, cuando lo hay, o en inglés); el volumen y el capítulo quedan los del archivo.</li>
  <li><b>Comic Vine</b> (comicvine.gamespot.com): fuerte en cómics estadounidenses, con el título de cada número. Pide una clave gratuita, pegada en <b>Opciones</b>.</li>
</ul>
<ol>
  <li>Haga clic en <b>Buscar la serie</b>. Los archivos se agrupan por serie (o use <b>Serie a buscar</b> para todos a la vez). Cuando solo una serie tiene ese nombre (y año) y números suficientes para los de los archivos, se elige sola; si no, usted elige entre las publicaciones, que muestran país, idioma, año de inicio y cantidad de números. Si el nombre completo no encuentra nada (<i>Serie: Historia</i>), la búsqueda prueba solo la serie.</li>
  <li>Cada archivo se empareja con su número por la numeración (o por el volumen, en un manga numerado por volúmenes; un número único, con el único número de la serie). Los que no coinciden quedan con el nombre de la serie y su propio número, en la misma carpeta que los demás, y abren la revisión, con la lista de números de la serie, para elegir el número exacto. <b>Ninguno de estos</b> devuelve el nombre que dice el archivo.</li>
</ol>
<ul>
  <li><b>Plantilla lista</b>: <b>Predeterminada de Renome</b> (que sirve también para Komga, que toma cada carpeta como una serie) o <b>Kavita</b> (sin el título después de " - ", que Kavita no lee). Las dos rellenan la plantilla y las carpetas y activan las carpetas de serie, porque los dos servidores quieren cada serie en su carpeta. Después se pueden cambiar libremente.</li>
  <li><b>Plantilla</b>: <code>{serie}</code>, <code>{año de la serie}</code>, <code>{número}</code> (tres dígitos: <code>012</code>), <code>{volumen}</code> (dos), <code>{capítulo}</code> (tres), <code>{título}</code> (del número), <code>{año}</code> (del número), <code>{total}</code>, <code>{editorial}</code>, <code>{guion}</code>, <code>{dibujo}</code> e <code>{id}</code> (del catálogo). La predeterminada es <code>{serie} v{volumen} #{número} c{capítulo} - {título} ({año})</code>: lo que falta se va junto con la marca pegada a ello (<code>v</code>, <code>#</code>, <code>c</code>), terminada en punto (<code>Vol. </code>) o el guion anterior. Así, un número estadounidense queda <code>Serie #012 (2019)</code>, un volumen de manga, <code>Serie v03</code>, y un tomo recopilatorio con título, <code>Serie v01 - Título (2018)</code>.</li>
  <li><b>Organizar en carpetas de serie</b>: cada cómic va a <code>{serie} ({año de la serie})</code> (la plantilla de las carpetas se puede cambiar; cada <code>/</code> es un nivel). Un cómic que ya está en una carpeta con el nombre de su serie se queda donde está, aunque la carpeta tenga otro formato (<code>Nombre de la Serie Vol. 2 (1988-1998)</code>, <code>Nombre de la Serie/Vol. 1</code>, <code>Nombre de la Serie</code> para un especial <code>Nombre de la Serie - Especial</code>, o una grafía un poco distinta). Los cómics de la lista se organizan juntos, como una biblioteca:<ul><li>cada serie queda con una sola grafía, la más común en la lista (mayúsculas, acentos y una letra cambiada en una palabra, como en <code>Apocalipse</code> y <code>Apocalypse</code>, cuentan como la misma serie), en el nombre de la carpeta y en el de los archivos;</li><li>un especial o número único va a la carpeta de la serie con cuyo nombre empieza el suyo (<code>Nombre de la Serie - Especial</code> en <code>Nombre de la Serie</code>; si hay varias, la de nombre más largo);</li><li>los números únicos con la misma parte antes del " - " (<code>Nombre de la Serie - Álbum Uno</code>, <code>Nombre de la Serie - Álbum Dos</code>) van juntos a una carpeta con esa parte;</li><li>los demás van cada uno a la carpeta de su serie, aunque sea un solo archivo.</li></ul>Deshacer lo devuelve todo a su sitio.</li>
</ul>
<h4>Limitaciones</h4>
<ul>
  <li>Los archivos <b>CBR</b> (RAR) se renombran solo por el nombre del archivo: no hay lector de RAR con licencia libre para que el programa lea lo que hay dentro. En la práctica, casi ningún CBR trae <code>ComicInfo.xml</code>.</li>
  <li>Los nombres de archivo muy fuera de lo común pueden leerse mal (un número que forma parte del nombre de la serie, por ejemplo). Revise la columna <b>Nuevo nombre</b> antes de renombrar.</li>
  <li>Los catálogos tienen huecos: las ediciones brasileñas y de otros países pueden faltar en Comic Vine, y el GCD, hecho por voluntarios, puede estar incompleto. Cuando no esté la serie correcta, sáltela: los archivos quedan con el nombre que dicen ellos mismos. La búsqueda no siempre encuentra una serie por un nombre traducido; en el selector, puede buscar de nuevo con otro nombre (el original, por ejemplo).</li>
  <li>En el GCD, la revisión muestra solo la numeración, sin títulos ni portadas: traerlos costaría una consulta por número. Una serie con el mismo nombre en otro idioma (la edición francesa, cuando hay una en el suyo) no se elige sola.</li>
  <li>El GCD y Comic Vine cuentan <b>números y volúmenes</b>, no capítulos: los capítulos sueltos de manga no se buscan y quedan con el nombre leído del archivo. MangaDex solo da la serie.</li>
  <li><b>Límites de consultas</b>: el GCD acepta unas 25 por minuto con cuenta (unas 30 por hora sin cuenta); Comic Vine, unas 200 por hora, solo para uso no comercial. El programa usa pocas por serie y guarda las respuestas mientras está abierto; si llega a un límite, espere un poco y busque de nuevo.</li>
  <li>El archivo del cómic nunca cambia: su <code>ComicInfo.xml</code> solo se lee.</li>
</ul>
<p class="tip">A los catálogos solo van el nombre de la serie buscada y los números de las series y los números, nunca el nombre completo del archivo, las carpetas ni el contenido. El acceso del GCD solo va a comics.org.</p>`,
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
  <li>un archivo que cambia de carpeta recibe un icono de carpeta en el nombre nuevo, y la columna <b>Carpeta</b> muestra, en azul, adónde va (<code>→ Artista/Álbum (2019)</code>).</li>
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
<p>En otros sistemas, esos caracteres de Windows solo generan un aviso amarillo, que se puede desactivar en Opciones. En Windows, macOS y los discos que no distinguen mayúsculas (exFAT, FAT, NTFS, como los pendrives), los nombres que solo difieren en mayúsculas cuentan como iguales.</p>
<p><b>Exportar lista</b>, en el pie, guarda la lista tal como se ve (carpeta, nombre actual, nombre nuevo, carpeta a la que va, estado y problema) en un archivo CSV, que se abre en cualquier hoja de cálculo. Sirve para revisar con calma un lote grande antes de renombrar.</p>
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
  <li>Las hojas de pistas y listas de reproducción (<code>.cue</code>, <code>.m3u</code>, <code>.m3u8</code>) que citan archivos renombrados o movidos se actualizan con los nuevos nombres. Un <code>.cue</code> que solo cita archivos inexistentes (el disco entero en un archivo que luego se dividió en pistas) se rehace con una pista por archivo, emparejando cada <code>TRACK</code> con la canción de la carpeta que tiene ese número en las etiquetas; si no es posible, se borra. Las listas <code>.m3u</code> en las que no existe ninguna canción también se borran. Son los únicos cambios que Renome hace en archivos; el contenido original queda guardado en el historial, y deshacer lo devuelve.</li>
  <li>Al terminar, un aviso en la esquina inferior derecha indica cuántos archivos se renombraron y desaparece solo.</li>
  <li>Si un archivo falla (sin permiso, borrado por otro programa…), los demás siguen, y al final aparece la lista de lo que falló y por qué.</li>
  <li>Después de renombrar, las reglas se desactivan (se puede cambiar en Opciones), para no aplicarlas dos veces.</li>
</ul>
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
  <li><b>Avisar sobre nombres que Windows no acepta</b>: caracteres como <code>:</code> y <code>?</code>, punto o espacio al final, y nombres reservados como <code>CON</code> y <code>NUL</code>. Útil para archivos que van a un pendrive o a Windows.</li>
</ul>
<p>Las opciones se guardan en <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) o <code>~/Library/Application Support/renome</code> (macOS). El historial está en <code>~/.local/share/renome/history</code> en Linux y en la misma carpeta de las opciones en los demás sistemas.</p>`,
  },
];
