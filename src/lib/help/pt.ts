import { ex, type Section } from "./types";

export const pt: Section[] = [
  {
    id: "start",
    title: "Primeiros passos",
    html: `
<p>O Renome renomeia muitos arquivos de uma vez. O trabalho tem três etapas:</p>
<ol>
  <li><b>Adicione os arquivos</b>: arraste arquivos ou pastas para a janela, ou use <b>+ Adicionar</b>.</li>
  <li><b>Monte as regras</b>: em <b>+ Regra</b>, escolha o que fazer com os nomes (substituir texto, numerar, inserir uma data…). A coluna <b>Novo nome</b> mostra o resultado na hora, antes de qualquer alteração no disco.</li>
  <li><b>Renomeie</b>: clique em <b>Renomear</b> (ou <kbd>Ctrl</kbd>+<kbd>Enter</kbd>). Se algo sair errado, desfaça pelo <b>Histórico</b>.</li>
</ol>
<p>Nada é alterado no disco até você clicar em Renomear. Pode experimentar à vontade.</p>`,
  },
  {
    id: "files",
    title: "Lista de arquivos",
    html: `
<h4>Adicionar</h4>
<p>O botão <b>+ Adicionar</b> tem três opções:</p>
<ul>
  <li><b>Arquivos…</b>: escolha um ou vários arquivos.</li>
  <li><b>Pastas (com subpastas)…</b>: adiciona todos os arquivos de dentro das pastas, inclusive das subpastas.</li>
  <li><b>Pastas (renomear as próprias pastas)…</b>: adiciona as pastas em si, para mudar o nome delas. Elas aparecem com um ícone de pasta.</li>
</ul>
<p>Arrastar para a janela funciona como "Arquivos" e "Pastas (com subpastas)". Arquivos que já estão na lista são ignorados, e os recém-adicionados ficam selecionados.</p>
<p>Arquivos ocultos (que começam com ponto, como <code>.config</code>) ficam de fora ao adicionar pastas, a não ser que você ligue a opção correspondente.</p>
<p>Também dá para abrir o programa já com arquivos: <code>renome foto1.jpg foto2.jpg pasta/</code>.</p>

<h4>Filtrar</h4>
<p>O campo <b>Filtrar</b> mostra só os arquivos que combinam. <b>Só os arquivos visíveis são renomeados</b>, e a numeração conta só eles.</p>
<ul>
  <li>Termos separados por vírgula ou espaço: <code>jpg, férias</code> mostra os nomes que contêm "jpg" <i>ou</i> "férias".</li>
  <li><code>*</code> e <code>?</code> são curingas: <code>*.png</code> (termina em .png), <code>IMG_????.jpg</code> (quatro caracteres quaisquer).</li>
  <li>Entre aspas, procura a frase inteira: <code>"breaking bad"</code>.</li>
  <li>Com <code>-</code> na frente, esconde o que combina: <code>-*.srt</code> esconde as legendas; <code>*.mkv -sample</code> mostra os .mkv, menos as amostras.</li>
  <li>Maiúsculas e acentos não importam: <code>ferias</code> encontra "Férias".</li>
</ul>

<h4>Ordenar e organizar</h4>
<ul>
  <li>Clique no título de uma coluna para ordenar; clique de novo para inverter.</li>
  <li>A ordem padrão é por <b>Novo nome</b>: a lista se reorganiza enquanto você mexe nas regras.</li>
  <li>Selecione com clique, <kbd>Ctrl</kbd>+clique (vários) e <kbd>Shift</kbd>+clique (intervalo).</li>
  <li><b>⤒ ⤓</b> movem os selecionados para cima ou para baixo, útil para acertar a numeração. Não funcionam na ordem por Novo nome.</li>
  <li><b>Remover</b> (ou <kbd>Delete</kbd>) tira os selecionados da lista; <b>Limpar</b> esvazia a lista. Os arquivos no disco não são tocados.</li>
  <li><kbd>Ctrl</kbd>+<kbd>Z</kbd> desfaz a última alteração na lista.</li>
</ul>`,
  },
  {
    id: "rules",
    title: "Como as regras funcionam",
    html: `
<p>As regras são aplicadas <b>de cima para baixo</b>, cada uma sobre o resultado da anterior. A ordem importa: numerar e depois trocar maiúsculas não dá o mesmo resultado que o contrário.</p>
<ul>
  <li>O <b>checkbox</b> no título liga e desliga a regra sem apagá-la.</li>
  <li><b>↑ ↓</b> mudam a ordem; <b>✕</b> remove a regra; clicar no título recolhe o card.</li>
  <li>Uma regra com erro (expressão inválida, por exemplo) fica com borda vermelha e é ignorada até ser corrigida.</li>
</ul>
<h4>Aplicar em</h4>
<p>Várias regras perguntam onde agir:</p>
<ul>
  <li><b>Nome (sem extensão)</b>: o padrão. Em <code>foto.jpg</code>, age só em <code>foto</code>.</li>
  <li><b>Extensão</b>: só em <code>jpg</code>.</li>
  <li><b>Nome completo</b>: em <code>foto.jpg</code> inteiro.</li>
</ul>
<p>Pastas não têm extensão: em <code>Série.2020</code>, o <code>.2020</code> faz parte do nome.</p>
<h4>Salvar e abrir regras</h4>
<p>📂 abre e 💾 salva o conjunto de regras num arquivo <code>.json</code>, para reaproveitar depois. Por padrão, o programa também lembra as regras ao ser fechado.</p>
<h4>Posições</h4>
<p>Onde uma regra pede uma posição, a contagem é de caracteres visíveis a partir de 1. Letras acentuadas e emojis contam como um caractere.</p>`,
  },
  {
    id: "r-replace",
    title: "Substituir texto",
    sub: true,
    html: `
<p>Troca um texto por outro.</p>
${ex("foto_de_ferias.jpg", "foto de ferias.jpg")}
<ul>
  <li><b>Localizar</b> e <b>Substituir por</b>: deixe "Substituir por" vazio para apagar o texto.</li>
  <li><b>Diferenciar maiúsculas</b>: desmarcado, "Foto" também encontra "foto" e "FOTO".</li>
  <li><b>Só a 1ª ocorrência em cada nome</b>: troca só a primeira vez que o texto aparece <i>em cada nome</i>. Em <code>a_b_c</code>, trocar <code>_</code> por espaço dá <code>a b_c</code>. Se o texto só aparece uma vez, todos os nomes mudam do mesmo jeito.</li>
</ul>`,
  },
  {
    id: "r-regex",
    title: "Expressão regular",
    sub: true,
    html: `
<p>Para padrões que o "Substituir texto" não alcança. Os grupos entre parênteses podem ser reaproveitados na substituição com <code>$1</code>, <code>$2</code>…</p>
${ex("01 - Abertura.mp3", "Abertura (01).mp3")}
<p>Expressão <code>(\\d+) - (.*)</code>, substituir por <code>$2 ($1)</code>.</p>
<table>
  <tr><td><code>.</code></td><td>qualquer caractere</td></tr>
  <tr><td><code>\\d</code> <code>\\s</code> <code>\\w</code></td><td>dígito, espaço, letra/dígito/_</td></tr>
  <tr><td><code>*</code> <code>+</code> <code>?</code></td><td>zero ou mais, um ou mais, opcional</td></tr>
  <tr><td><code>{2}</code> <code>{1,3}</code></td><td>exatamente 2, de 1 a 3 vezes</td></tr>
  <tr><td><code>[abc]</code> <code>[^abc]</code></td><td>um desses caracteres, qualquer um menos esses</td></tr>
  <tr><td><code>^</code> <code>$</code></td><td>início e fim do texto</td></tr>
  <tr><td><code>( )</code></td><td>grupo, reaproveitado como <code>$1</code>, <code>$2</code>…</td></tr>
  <tr><td><code>\\.</code> <code>\\(</code></td><td>o próprio ponto, o próprio parêntese</td></tr>
</table>
<p class="tip">Se um grupo é seguido de letras ou dígitos, use chaves: <code>\${1}a</code>. Escrito <code>$1a</code>, o programa procura um grupo chamado "1a", que não existe.</p>
<p>Grupos com nome também funcionam: <code>(?P&lt;ano&gt;\\d{4})</code> e depois <code>\${ano}</code>.</p>`,
  },
  {
    id: "r-rearrange",
    title: "Reorganizar partes",
    sub: true,
    html: `
<p>Divide o nome num separador e monta de novo em outra ordem. Não precisa de expressões regulares.</p>
${ex("Série - Episódio - S01E02.mkv", "Série - S01E02 - Episódio.mkv")}
<ul>
  <li><b>Separador</b>: o texto que divide as partes (aqui, <code> - </code>). Espaços em volta de cada parte são removidos, e partes vazias são descartadas.</li>
  <li>O card mostra as partes do primeiro arquivo da lista, numeradas. <b>Clique numa parte</b> para acrescentá-la à nova ordem.</li>
  <li><b>Nova ordem</b>: números são as partes; todo o resto é mantido como texto. Para o exemplo: <code>1 - 3 - 2</code>.</li>
  <li>Partes que não aparecem na ordem são removidas. Ordem <code>2 (1)</code> em <code>Silva - Ana</code> dá <code>Ana (Silva)</code>.</li>
  <li>Se um nome tem menos partes do que a ordem pede, ele fica como está.</li>
</ul>`,
  },
  {
    id: "r-case",
    title: "Maiúsculas / minúsculas",
    sub: true,
    html: `
<ul>
  <li><b>minúsculas</b>: <code>Foto De Férias</code> → <code>foto de férias</code></li>
  <li><b>MAIÚSCULAS</b>: → <code>FOTO DE FÉRIAS</code></li>
  <li><b>Primeira letra maiúscula</b>: → <code>Foto de férias</code></li>
  <li><b>Cada Palavra Maiúscula</b>: → <code>Foto De Férias</code></li>
  <li><b>Cada Palavra, exceto de/da/e/of…</b>: → <code>Foto de Férias</code>. Palavras curtas de ligação (de, da, do, e, em, para, of, the, and, del, la, y…) ficam em minúsculas, a não ser no início do nome ou de uma parte (depois de <code> - </code>, <code>:</code> ou <code>(</code>).</li>
</ul>
<p>Nos dois modos "Cada Palavra", algarismos romanos ficam em maiúsculas (<code>Rocky III</code>, <code>Parte II</code>) e palavras com maiúsculas no meio ficam como estão (<code>iPhone</code>, <code>McDonald's</code>).</p>
<p>Use "Aplicar em: Extensão" para mudar só a extensão.</p>`,
  },
  {
    id: "r-insert",
    title: "Inserir",
    sub: true,
    html: `
<p>Acrescenta algo ao nome.</p>
<h4>O quê</h4>
<ul>
  <li><b>Texto</b>: um texto fixo.</li>
  <li><b>Data atual</b>: a data de hoje.</li>
  <li><b>Data de modificação do arquivo</b>.</li>
  <li><b>Data em que a foto foi tirada</b>: lida dos dados EXIF da foto (JPEG, HEIC, PNG, WebP, TIFF e formatos RAW). Fotos sem essa data ficam como estão, a não ser que <b>"Sem essa data, usar a de modificação"</b> esteja marcado. Vídeos ainda não são lidos.</li>
  <li><b>Nome da pasta</b>: o nome da pasta onde o arquivo está.</li>
</ul>
${ex("IMG_1234.jpg", "2024-07-15 IMG_1234.jpg")}
<h4>Formato da data</h4>
<table>
  <tr><td><code>%Y</code> <code>%y</code></td><td>ano com 4 ou 2 dígitos (2024, 24)</td></tr>
  <tr><td><code>%m</code> <code>%d</code></td><td>mês e dia (07, 15)</td></tr>
  <tr><td><code>%H</code> <code>%M</code> <code>%S</code></td><td>hora, minuto, segundo</td></tr>
  <tr><td><code>%j</code></td><td>dia do ano (001–366)</td></tr>
</table>
<p>Exemplo: <code>%Y-%m-%d %H.%M.%S</code> dá <code>2024-07-15 14.32.10</code>. Uma <code>/</code> no formato vira <code>-</code>, porque não pode haver barra em nomes de arquivo.</p>
<h4>Onde</h4>
<ul>
  <li><b>No início</b> ou <b>No fim</b> do nome (antes da extensão).</li>
  <li><b>Na posição</b> N: antes do N-ésimo caractere (1 = no início).</li>
  <li><b>Na posição (contar do fim)</b> N: N caracteres antes do fim (0 = no fim, 1 = antes do último).</li>
</ul>`,
  },
  {
    id: "r-number",
    title: "Numerar",
    sub: true,
    html: `
<p>Numera os arquivos na ordem da lista.</p>
${ex("praia.jpg", "Viagem 01.jpg")}
<ul>
  <li><b>Modelo</b>: o novo nome, com variáveis:
    <ul>
      <li><code>{n}</code>: o número;</li>
      <li><code>{nome}</code>: o nome atual (sem extensão);</li>
      <li><code>{pasta}</code>: o nome da pasta onde o arquivo está.</li>
    </ul>
    Exemplos: <code>Viagem {n}</code>, <code>{nome} - {n}</code>, <code>{pasta} {n}</code>.</li>
  <li><b>Início</b> e <b>Passo</b>: 1 e 1 dão 1, 2, 3…; 10 e 10 dão 10, 20, 30…; passo negativo conta para trás.</li>
  <li><b>Dígitos</b>: completa com zeros à esquerda (2 dígitos: 01, 02… 10).</li>
  <li><b>Recomeçar a numeração em cada pasta</b>: cada pasta começa do início.</li>
</ul>
<p>A numeração segue a ordem da lista e conta só os arquivos visíveis no filtro. Na ordem por Novo nome, segue a ordem do nome atual. Para controlar a ordem, ordene por outra coluna ou use ⤒ ⤓.</p>`,
  },
  {
    id: "r-remove",
    title: "Remover caracteres",
    sub: true,
    html: `
<ul>
  <li><b>Espaços repetidos e nas pontas</b>: <code>  foto   de  férias </code> → <code>foto de férias</code></li>
  <li><b>Todos os espaços</b>: → <code>fotodeférias</code></li>
  <li><b>Acentos</b>: <code>Canção</code> → <code>Cancao</code></li>
  <li><b>Dígitos</b>: <code>foto123</code> → <code>foto</code></li>
  <li><b>Caracteres</b>: cada caractere digitado é removido onde aparecer. <code>_#</code> remove todos os <code>_</code> e <code>#</code>.</li>
</ul>`,
  },
  {
    id: "r-range",
    title: "Remover trecho",
    sub: true,
    html: `
<p>Apaga um pedaço do nome, definido pelo começo e pelo fim.</p>
<ul>
  <li><b>De</b>: o início do nome, uma posição, ou a primeira vez que um texto aparece.</li>
  <li><b>Até</b>: o fim do nome, uma posição, ou um texto (procurado depois do começo).</li>
  <li><b>Remover também os textos delimitadores</b>: se os próprios textos de início e fim também saem.</li>
</ul>
${ex("Filme [1080p] (2020).mkv", "Filme  (2020).mkv")}
<p>De texto <code>[</code> até texto <code>]</code>, removendo os delimitadores. Os textos diferenciam maiúsculas. Se um deles não aparece no nome, o nome fica como está.</p>`,
  },
  {
    id: "r-char",
    title: "Caractere em posição",
    sub: true,
    html: `
<p>Troca ou apaga o caractere numa posição.</p>
<ul>
  <li><b>Posição</b>: a partir de 1. Com <b>Contar do fim</b>, 1 é o último caractere.</li>
  <li><b>Só se for</b>: só muda se o caractere naquela posição for este (vazio = qualquer). Não diferencia maiúsculas, a não ser que <b>Diferenciar maiúsculas</b> esteja marcado.</li>
  <li><b>Trocar por</b>: o novo caractere. Vazio apaga.</li>
</ul>
${ex("_foto.jpg", "foto.jpg")}
<p>Posição 1, só se for <code>_</code>, trocar por vazio.</p>`,
  },
  {
    id: "r-ext",
    title: "Extensão",
    sub: true,
    html: `
<ul>
  <li><b>minúsculas</b> / <b>MAIÚSCULAS</b>: <code>foto.JPG</code> → <code>foto.jpg</code></li>
  <li><b>Definir</b>: troca a extensão por outra (<code>jpeg</code> → <code>jpg</code>). Não converte o arquivo, só muda o nome.</li>
  <li><b>Remover</b>: tira a extensão.</li>
</ul>
<p>Pastas não são afetadas por esta regra.</p>`,
  },
  {
    id: "r-movie",
    title: "Filme (TMDB)",
    sub: true,
    html: `
<p>Dá a cada filme o nome oficial do <b>TMDB</b> (themoviedb.org), no formato que o Jellyfin reconhece. Precisa de uma <b>chave da API do TMDB</b>, gratuita, informada nas Opções.</p>
${ex("O.Senhor.dos.Aneis.2001.1080p.BluRay.x264.mkv", "O Senhor dos Anéis - A Sociedade do Anel (2001) [tmdbid-120].mkv")}
<ol>
  <li>Adicione a regra e clique em <b>Buscar no TMDB</b>. O programa tira o título e o ano do nome de cada arquivo (pontos viram espaços; somem colchetes, parênteses e marcas como 1080p, BluRay, x264) e procura no TMDB.</li>
  <li>Quando o título e o ano batem sem dúvida, o filme é escolhido sozinho (<b>✓</b> ao lado do novo nome). Quando há dúvida (<b>?</b>), abre a janela de revisão: clique no filme certo (ou aperte 1–9). O ano do arquivo pode estar um ano antes ou depois do TMDB; isso é levado em conta.</li>
  <li>Se nenhum servir, corrija o título ou o ano no campo de busca da janela, ou marque <b>Nenhum destes</b> (<b>✕</b>: o arquivo fica como está).</li>
</ol>
<ul>
  <li><b>Modelo</b>: <code>{título}</code>, <code>{original}</code> (título original), <code>{ano}</code> e <code>{id}</code>. O padrão é <code>{título} ({ano}) [tmdbid-{id}]</code>.</li>
  <li><b>Trocar ":" por</b>: dois-pontos não são aceitos no Windows; por padrão viram <code> - </code>. Barras viram hífens e <code>? * &lt; &gt; |</code> são removidos.</li>
  <li>Legendas e outros arquivos com o mesmo nome do filme recebem a mesma escolha. Legendas mantêm o idioma no fim do nome: <code>Matrix (1999) [tmdbid-603].pt-BR.srt</code>.</li>
  <li><b>Criar uma pasta para cada filme</b>: o filme e as legendas vão para uma pasta com o novo nome, como o Jellyfin recomenda: <code>Filmes/Matrix (1999) [tmdbid-603]/Matrix (1999) [tmdbid-603].mkv</code>. Um filme que já está na pasta certa não é movido de novo. Desfazer devolve os arquivos e apaga a pasta, se ela ficar vazia.</li>
  <li>Para tirar os acentos, acrescente a regra <b>Remover caracteres → Acentos</b> depois desta.</li>
</ul>
<h4>Metadados (.nfo e pôster)</h4>
<p>Com <b>Salvar .nfo e pôster ao lado do filme</b>, ao renomear o programa cria, junto de cada vídeo, <code>Nome (ano) [tmdbid-…].nfo</code> e o pôster; opcionalmente também a imagem de fundo (fanart) e o logo do filme. Dentro da pasta do filme, as imagens se chamam <code>poster.jpg</code>, <code>fanart.jpg</code> e <code>logo.png</code>; soltas, levam o nome do filme na frente (<code>…-poster.jpg</code>). Jellyfin, Kodi e Emby leem esses arquivos.</p>
<p>O <code>.nfo</code> traz tudo o que o TMDB tem e o Jellyfin lê: título e título original, ano e data de lançamento, sinopse e frase de efeito, duração, nota e votos, classificação indicativa do país do idioma escolhido (ex.: BR-12), gêneros, palavras-chave (como etiquetas), países, estúdios, coleção (ex.: "O Senhor dos Anéis – Coleção"), trailer do YouTube, direção, roteiro, elenco com personagens e fotos, produtores, compositor e os IDs do TMDB e do IMDb. O arquivo do filme não é alterado, arquivos que já existem não são substituídos, e <b>desfazer</b> a renomeação apaga o que foi criado.</p>
<p>Filmes que <b>já têm o nome certo</b> também recebem os metadados: o botão Renomear os conta, e a confirmação avisa. Se o nome já traz <code>[tmdbid-…]</code>, o filme é reconhecido direto pelo ID, sem busca. Assim dá para completar uma biblioteca já organizada só com .nfo e imagens.</p>
<p class="tip">Só o título e o ano procurados são enviados ao TMDB, nunca o nome completo do arquivo nem as pastas.</p>`,
  },
  {
    id: "r-series",
    title: "Série (TMDB)",
    sub: true,
    html: `
<p>Dá a cada episódio o nome oficial do <b>TMDB</b>, com temporada, número e título, no formato que o Jellyfin reconhece. Usa a mesma chave da API da regra de filmes.</p>
${ex("Breaking.Bad.S01E03.720p.WEB-DL.mkv", "Breaking Bad (2008) - S01E03 - ...E o Saco Está no Rio.mkv")}
<ol>
  <li>No campo <b>Série</b>, digite o nome, o ID do TMDB ou cole o link da página da série (<code>themoviedb.org/tv/1396-…</code>). Vazio, o programa usa o nome que os arquivos têm em comum, ou o nome da pasta.</li>
  <li>Clique em <b>Buscar episódios</b>. Se houver mais de uma série parecida, escolha a certa. O programa baixa a lista completa de episódios e procura o de cada arquivo visível.</li>
  <li>Os episódios encontrados com segurança ganham <b>✓</b>. Os duvidosos (<b>?</b>) e os não encontrados (<b>✕</b>) vão para a janela de revisão.</li>
</ol>
<h4>Como cada episódio é encontrado</h4>
<ul>
  <li><b>Pelo número</b>, em vários formatos: <code>S01E02</code>, <code>s1e2</code>, <code>1x02</code>, <code>T1 E2</code>, <code>Temporada 1 Episódio 2</code>, <code>Ep 02</code> (a temporada vem da pasta, como <code>Season 2</code> ou <code>Temporada 2</code>), numeração corrida (<code>Naruto - 125</code>), <code>201</code> (temporada 2, episódio 1) e data de exibição (<code>2024.03.15</code>). Arquivos com dois episódios (<code>S01E01E02</code>, <code>S01E01-02</code>) viram <code>S01E01-E02</code>.</li>
  <li><b>Pelo título</b>: o texto depois do número é comparado com os títulos de todos os episódios, no seu idioma, em inglês e no idioma original, ignorando acentos, maiúsculas e pequenos erros de grafia. Sem número nenhum, o título sozinho basta, se for claro.</li>
  <li><b>Quando o número e o título discordam</b> (o nome diz S02E08, mas o título é o do S01E03), o arquivo fica com <b>?</b>, e a revisão mostra as duas opções, sugerindo a do título.</li>
</ul>
<h4>Revisão</h4>
<p>Mostra o que foi lido no nome, as sugestões (clique, aperte 1–9, ou <b>Enter</b> para a primeira) e todos os episódios da série por temporada, com um filtro por título ou número (<code>s01e03</code>). <b>Ctrl+clique</b> marca mais de um episódio da mesma temporada, para um arquivo com episódios juntos. Legendas lidas do mesmo jeito recebem a mesma escolha.</p>
<ul>
  <li><b>Modelo</b>: <code>{série}</code>, <code>{original}</code> (nome original), <code>{ano}</code> (estreia), <code>{temporada}</code>, <code>{episódio}</code> (com dois dígitos), <code>{título}</code> e <code>{id}</code>. O padrão é <code>{série} ({ano}) - S{temporada}E{episódio} - {título}</code>.</li>
  <li>Episódios sem tradução recebem o título em inglês (ou no idioma original), em vez de "Episódio 4".</li>
  <li>Legendas mantêm o idioma no fim do nome: <code>….pt-BR.srt</code>.</li>
  <li>Os arquivos que não estão na série escolhida ficam como estão. Para várias séries, filtre a lista e busque uma de cada vez: cada arquivo guarda a sua.</li>
</ul>
<h4>Pastas da série e da temporada</h4>
<p>Com <b>Organizar em pastas da série e da temporada</b>, cada episódio vai para a estrutura que o Jellyfin recomenda: <code>Séries/Breaking Bad (2008) [tmdbid-1396]/Season 01/…</code>. A pasta da série é criada ao lado da pasta onde a série já está (se os arquivos estão em <code>Breaking Bad/Season 1</code>, a nova pasta fica ao lado de <code>Breaking Bad</code>, não dentro dela); com os arquivos soltos, dentro da pasta deles. O nome da pasta da série tem o próprio modelo, com as mesmas variáveis (padrão <code>{série} ({ano}) [tmdbid-{id}]</code>; use <code>{original}</code> para o nome original). As pastas antigas que ficarem vazias são apagadas; as que ainda têm outros arquivos ficam. <b>Desfazer</b> devolve tudo ao lugar, recriando as pastas antigas.</p>
<h4>Metadados (.nfo e imagens)</h4>
<p>Com <b>Salvar .nfo e imagens para o Jellyfin</b>, ao renomear o programa cria:</p>
<ul>
  <li>na <b>pasta da série</b>: <code>tvshow.nfo</code> (título, título original, ano, sinopse, situação, duração, nota, classificação indicativa, gêneros, palavras-chave, emissoras e estúdios, criadores, trailer, elenco com personagens e os IDs do TMDB, IMDb e TVDB), <code>poster.jpg</code> e o pôster de cada temporada (<code>season01-poster.jpg</code>); opcionalmente <code>fanart.jpg</code> e <code>logo.png</code>;</li>
  <li>na <b>pasta da temporada</b>: <code>season.nfo</code>, com o nome e a sinopse da temporada;</li>
  <li>ao lado de <b>cada episódio</b>: <code>….nfo</code> (título, temporada e número, data de exibição, sinopse, duração, nota, direção, roteiro e atores convidados) e a miniatura <code>…-thumb.jpg</code>.</li>
</ul>
<p>Os arquivos da série e da temporada só são criados quando os episódios estão numa pasta da série (com o nome dela, ou com <code>[tmdbid-…]</code>), seja a de antes, seja a criada pela opção de pastas. Episódios que já têm o nome certo também recebem os metadados. Arquivos que já existem não são substituídos, e desfazer apaga o que foi criado.</p>
<p class="tip">Só o nome da série é enviado ao TMDB, nunca o nome dos arquivos nem as pastas.</p>`,
  },
  {
    id: "r-music",
    title: "Música (tags)",
    sub: true,
    html: `
<p>Dá a cada música o nome que vem das <b>tags</b> gravadas no arquivo (ID3 do MP3, Vorbis do FLAC e do OGG, MP4 do M4A e outras), sem precisar de internet.</p>
${ex("01.mp3", "01 - Shine On You Crazy Diamond.mp3")}
<ul>
  <li><b>Modelo</b>: <code>{faixa}</code>, <code>{título}</code>, <code>{artista}</code>, <code>{artista do álbum}</code>, <code>{álbum}</code>, <code>{ano}</code>, <code>{disco}</code>, <code>{gênero}</code> e <code>{compositor}</code>. O padrão é <code>{faixa} - {título}</code>. A faixa tem dois dígitos (três em álbuns com mais de 99 faixas).</li>
  <li>Tags vazias não deixam sobras: com <code>{faixa} - {artista} - {título}</code>, uma música sem artista vira <code>01 - Título</code>.</li>
  <li><b>Número do disco na faixa</b>: em álbuns com mais de um disco, a faixa vira <code>1-01</code>, <code>2-01</code>…, para os discos não se misturarem.</li>
  <li><b>O álbum como um todo</b>: as faixas de um mesmo álbum (mesma pasta, ou discos <code>CD 01</code>/<code>CD 02</code>, e mesmo nome de álbum) combinam os dados do álbum. Uma faixa sem ano recebe o das outras. Sem "artista do álbum" nas tags, vale o artista da maioria das faixas (desconsiderando participações, como <code>feat.</code>); sem maioria, o álbum é uma coletânea: <code>Various Artists</code>, como o Jellyfin reconhece. Assim, as faixas de um álbum nunca se espalham em pastas diferentes.</li>
  <li><b>Faixas bônus</b>: numa pasta em que a maioria das músicas é de um álbum, as faixas com outro álbum nas tags (um single, uma trilha sonora) e numeradas depois das do álbum (<code>15</code>, <code>16</code> depois de <code>01</code>–<code>14</code>) ficam com esse álbum.</li>
  <li><b>Singles</b>: uma música com o álbum <code>Single</code> nas tags tem o próprio título como álbum.</li>
  <li><b>Coletâneas</b>: num álbum em que a maioria das faixas é de outros artistas que não o do álbum, o artista entra no nome antes do título quando o modelo não o tem: <code>03 - Eric Burdon - Orange and Red Beams</code>.</li>
  <li>Se o nome do álbum já traz um ano (<code>Programa 12 (2025-05-04)</code>), o <code>({ano})</code> do modelo é omitido.</li>
  <li>Músicas <b>sem título nas tags</b> ficam como estão e ganham <b>✕</b> (a não ser que a busca as identifique pelo som, veja abaixo); o card mostra quantas são. Com <b>Completar pelo nome do arquivo</b>, o programa lê <code>01 - Artista - Título</code>, <code>01. Título</code> ou <code>1-05 Título</code> do nome, e o álbum e o ano da pasta (<code>Artista - Álbum (1975)</code>, ou só <code>Álbum</code>; uma pasta <code>CD2</code> dá o disco). As tags que existem têm preferência.</li>
</ul>
<h4>Pastas de artista e álbum</h4>
<p>Com <b>Organizar em pastas de artista e álbum</b>, cada música vai para <code>{artista do álbum}/{álbum} ({ano})</code> (o modelo das pastas pode ser trocado; cada <code>/</code> é um nível). Assim, <code>Música/Pink Floyd/WYWH/CD1/01.mp3</code> vira <code>Música/Pink Floyd/Wish You Were Here (1975)/01 - Shine On….mp3</code>: as pastas que já levam o nome do artista, do álbum ou do disco (ou que estão dentro da pasta do artista) são substituídas, não ganham pastas novas dentro. Arquivos soltos ganham as pastas dentro da pasta onde estão. Pequenas diferenças de grafia entre a pasta e a tag (<code>Сага об</code> e <code>Сага о</code>) não atrapalham. Uma pasta que já reúne álbuns com o nome dela (<code>Programa (1000 - ..)</code> para os álbuns "Programa 1037 - …") é tratada como coleção: os álbuns ficam direto nela, sem pastas de artista no meio.</p>
<p><b>Cópias</b>: se duas cópias idênticas de um álbum (encontradas pelo programa, como em <b>Cópias idênticas</b>) iriam para o mesmo lugar, a primeira segue e a outra fica onde está, marcada como cópia.</p>
<ul>
  <li>Letras (<code>.lrc</code>) com o mesmo nome de uma música recebem o novo nome dela e vão junto.</li>
  <li>Capas, <code>.cue</code>, <code>.log</code> e outros arquivos da pasta do álbum vão junto com a maioria das músicas da pasta (músicas de outros álbuns que não sejam faixas bônus seguem o álbum delas). Subpastas sem músicas (<code>Artwork</code>, <code>Scans</code>) vão junto, com o mesmo nome; arquivos da pasta acima de <code>CD 01</code>/<code>CD 02</code> também. Se dois desses arquivos tiverem o mesmo nome (um <code>log.txt</code> em cada disco), ganham o nome da pasta de origem: <code>log (CD 01).txt</code>.</li>
  <li>Nomes de artista ou álbum que só diferem em maiúsculas (<code>a-ha</code> e <code>A-Ha</code>, de tags de álbuns diferentes) viram uma pasta só: a que já existe no disco ou a grafia mais usada.</li>
  <li>Pastas antigas que ficarem vazias são apagadas; desfazer devolve tudo ao lugar.</li>
  <li><b>Criar uma playlist por álbum</b>: depois de renomear, grava <code>&lt;nome da pasta&gt;.m3u8</code> na pasta de cada álbum, com as músicas (de todos os discos) na ordem dos discos e faixas. Vale também para os álbuns que já estão com os nomes certos (eles contam no botão <b>Renomear</b>). Não é criada se o álbum já tiver um <code>.cue</code> ou uma playlist, nem para uma música sozinha. Desfazer apaga a playlist criada.</li>
  <li>Arquivos <code>.cue</code> e listas <code>.m3u</code> passam a citar os novos nomes das músicas (veja <b>Renomear e desfazer</b>).</li>
</ul>
<h4>Corrigir pelo MusicBrainz</h4>
<p>Para tags erradas ou incompletas, <b>Buscar no MusicBrainz</b> identifica cada álbum no <b>MusicBrainz</b> (musicbrainz.org), um banco de dados aberto de música. Não precisa de chave.</p>
<ul>
  <li>As músicas visíveis são agrupadas por álbum. Se as tags já trazem o ID do MusicBrainz, o álbum é reconhecido direto (quando todas as músicas estão nele; senão, vai para a conferência); senão, o programa busca pelo artista e pelo nome do álbum.</li>
  <li>Um mesmo álbum costuma ter várias edições (original, remasterizada, de luxo, de outro país). O programa confere o <b>número de faixas</b>, o <b>título</b> e a <b>duração de cada uma</b> (uma faixa com o mesmo número mas outro título e outra duração não conta): se a melhor edição encontrada bate com as músicas, ela é escolhida sozinha (<b>✓</b>). Senão, abre a revisão, com capa, data, país, formato, gravadora e número de faixas (em verde quando é igual ao da pasta), para você escolher, ou <b>Nenhum</b> para manter as tags.</li>
  <li>Com o álbum escolhido, os nomes e as pastas usam os dados do MusicBrainz: título de cada faixa, artista, artista do álbum, nome do álbum e ano, com a grafia oficial. Cada música é casada com a sua faixa pelo disco e número (ou pelo título).</li>
  <li><b>Salvar a capa do álbum</b>: grava <code>cover.jpg</code> (do Cover Art Archive) na pasta do álbum, se ela ainda não tiver capa. Se a edição escolhida não tiver imagem (uma fita cassete, por exemplo), usa a capa do álbum. Desfazer apaga a capa criada.</li>
  <li>As consultas são espaçadas (uma a cada 2 segundos), como o MusicBrainz pede; cada álbum leva uns 4 segundos.</li>
</ul>
<h4>Identificar pelo som (AcoustID)</h4>
<p>Com <b>Identificar pelo som</b> marcado (o padrão), a busca também cuida das músicas <b>sem título nas tags</b> (mesmo com <b>Completar pelo nome do arquivo</b>, já que o nome é só um palpite) e dos álbuns que a busca pelo nome não achou ou deixou em dúvida. Num álbum em dúvida, o resultado pelo som só vale se identificar um lançamento com segurança; senão, ficam os candidatos da busca. O programa ouve os 2 primeiros minutos de cada música, calcula a <b>impressão digital</b> do som e pergunta ao <b>AcoustID</b> (acoustid.org), um banco aberto de impressões digitais ligado ao MusicBrainz, qual gravação é aquela.</p>
<ul>
  <li>As músicas de uma pasta são repartidas entre os álbuns em que as gravações aparecem: se a maioria está num mesmo álbum, a pasta é esse álbum; músicas avulsas vão para o álbum de estúdio do artista, e não para uma coletânea.</li>
  <li>Entre as edições, fica a com o número de faixas mais próximo do da pasta e, depois, a mais antiga. Se as faixas e as durações batem, o álbum é escolhido sozinho; senão, vai para a revisão, que mostra <b>Pelo som:</b> com o álbum ouvido.</li>
  <li>Uma música sem título recebe o nome da faixa do álbum escolhido. Uma música que o AcoustID não conhece aparece como <b>não reconhecida</b> e fica como está.</li>
  <li>Lê MP3, FLAC, OGG, M4A (AAC e ALAC), WAV e AIFF. Outros formatos, como Opus e WMA, não são ouvidos.</li>
  <li>Cada música leva cerca de meio segundo para ser ouvida e consultada.</li>
</ul>
<p class="tip">Ao AcoustID vão só a impressão digital do som e a duração de cada música, nunca o arquivo.</p>
<p class="tip">O Renome só lê as tags; o conteúdo das músicas não é alterado. Ao MusicBrainz vão só o artista e o nome do álbum.</p>`,
  },
  {
    id: "preview",
    title: "Pré-visualização e conflitos",
    html: `
<p>A coluna <b>Novo nome</b> mostra o resultado das regras na hora. Nas linhas que mudam:</p>
<ul>
  <li>no <b>Nome atual</b>, o que vai sair aparece <span class="del">em vermelho, riscado</span>;</li>
  <li>no <b>Novo nome</b>, o que entra aparece <span class="ins">em azul</span>.</li>
</ul>
<p>Na primeira coluna, ● indica que o arquivo será renomeado, <span class="bad">!</span> vermelho indica um conflito e <span class="warn">!</span> amarelo indica um aviso. Passe o mouse para ver o motivo.</p>
<h4>Editar à mão</h4>
<p>Clique duas vezes no novo nome (ou selecione um arquivo e aperte <kbd>F2</kbd>) para escrever o nome que quiser. <kbd>Enter</kbd> confirma e <kbd>Esc</kbd> cancela. Um nome editado à mão ignora as regras; ↺ volta a usar as regras.</p>
<h4>Conflitos</h4>
<p>Arquivos com conflito não são renomeados. Os motivos são:</p>
<ul>
  <li>nome vazio, ou contendo <code>/</code>;</li>
  <li>nome com mais de 255 bytes;</li>
  <li>dois arquivos ficariam com o mesmo nome na mesma pasta;</li>
  <li>já existe um arquivo com esse nome fora da lista (nada é sobrescrito);</li>
  <li>no Windows: os caracteres <code>\\ : * ? " &lt; &gt; |</code>, ou ponto ou espaço no fim do nome.</li>
</ul>
<p>Em outros sistemas, esses caracteres do Windows só geram um aviso amarelo, desligável nas Opções. No Windows e no macOS, nomes que só diferem em maiúsculas contam como iguais.</p>`,
  },
  {
    id: "identical",
    title: "Cópias idênticas",
    html: `
<p>Depois que você adiciona arquivos, o programa procura, em segundo plano, arquivos da lista com <b>conteúdo idêntico</b>, mesmo que tenham nomes diferentes. Nada é apagado nem alterado: é só um aviso.</p>
<ul>
  <li>Arquivos com cópia ganham uma etiqueta com um <b>número de grupo</b> ao lado do nome: arquivos com o mesmo número são idênticos entre si, mesmo com nomes diferentes.</li>
  <li><b>Selecione um arquivo</b> e as cópias dele ficam destacadas na lista. Passar o mouse sobre a etiqueta mostra os nomes das outras cópias.</li>
  <li>O rodapé mostra quantos arquivos têm cópia. <b>Clique nele</b> para ver só esses arquivos, com as cópias lado a lado; clique de novo para ver todos. Daí você pode removê-los da lista ou renomeá-los à parte.</li>
  <li>Quando duas cópias iriam ficar com o mesmo nome, o conflito avisa que os arquivos são idênticos.</li>
</ul>
<p>A busca é feita em etapas para ser rápida: só arquivos do mesmo tamanho são comparados; depois, só o começo e o fim; o arquivo inteiro só é lido quando tudo isso coincide. Com arquivos grandes, o rodapé mostra o progresso. Arquivos vazios, pastas e atalhos (links simbólicos) ficam de fora.</p>`,
  },
  {
    id: "apply",
    title: "Renomear e desfazer",
    html: `
<p><b>Renomear</b> (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) aplica os novos nomes dos arquivos visíveis que mudam e não têm conflito. Antes, o programa pede confirmação (desligável nas Opções).</p>
<ul>
  <li>A renomeação é feita em duas etapas, passando por um nome temporário. Por isso dá para trocar nomes entre si (a ↔ b), e nenhum arquivo é sobrescrito.</li>
  <li>Pastas e arquivos de dentro delas podem ser renomeados juntos: primeiro o conteúdo, depois a pasta.</li>
  <li>Folhas de faixas e listas de reprodução (<code>.cue</code>, <code>.m3u</code>, <code>.m3u8</code>) que citam arquivos renomeados ou movidos são atualizadas com os novos nomes. Um <code>.cue</code> que cita só arquivos inexistentes (o disco inteiro num arquivo que depois foi dividido em faixas) é refeito com uma faixa por arquivo, casando cada <code>TRACK</code> com a música da pasta que tem aquele número nas tags; se não der, é apagado. Listas <code>.m3u</code> em que nenhuma música existe também são apagadas. São as únicas alterações que o Renome faz em arquivos; o conteúdo original fica guardado no histórico, e desfazer o devolve.</li>
  <li>Ao terminar, um aviso no canto inferior direito confirma quantos arquivos foram renomeados e some sozinho.</li>
  <li>Se um arquivo falhar (sem permissão, apagado por outro programa…), os outros seguem, e no fim aparece a lista do que falhou e por quê.</li>
  <li>Depois de renomear, as regras são desligadas (desligável nas Opções), para não serem aplicadas duas vezes.</li>
</ul>
<h4>Histórico</h4>
<p>Cada renomeação fica registrada no <b>Histórico</b>, mesmo depois de fechar o programa. Lá você pode:</p>
<ul>
  <li><b>Desfazer</b>: volta os nomes anteriores.</li>
  <li><b>Tentar de novo</b>: aparece quando parte do desfazer falhou (por exemplo, um arquivo foi movido depois). Devolva o arquivo ao lugar e tente de novo.</li>
  <li><b>✕</b>: exclui a entrada do histórico. Os arquivos não mudam, mas aquela renomeação não poderá mais ser desfeita.</li>
  <li><b>Limpar histórico</b>: exclui todas as entradas de uma vez, com o mesmo efeito.</li>
</ul>
<p class="tip"><kbd>Ctrl</kbd>+<kbd>Z</kbd> desfaz alterações na <i>lista</i> (adicionar, remover, mover, editar nome). Para desfazer uma renomeação já feita, use o Histórico.</p>`,
  },
  {
    id: "recipes",
    title: "Receitas",
    html: `
<h4>Fotos com a data em que foram tiradas</h4>
<p>Inserir → Data em que a foto foi tirada, formato <code>%Y-%m-%d %H.%M.%S </code>, no início.</p>
${ex("IMG_1234.jpg", "2024-07-15 14.32.10 IMG_1234.jpg")}
<h4>Numerar as fotos de várias pastas</h4>
<p>Adicione as pastas (com subpastas). Numerar → modelo <code>{pasta} {n}</code>, 3 dígitos, recomeçar em cada pasta.</p>
${ex("Praia/IMG_0001.jpg", "Praia/Praia 001.jpg")}
<h4>Limpar nomes de episódios</h4>
<p>Substituir texto <code>.</code> por espaço (Nome sem extensão), depois Remover trecho de texto <code> 720p</code> até o fim.</p>
${ex("Serie.S01E02.720p.WEB.mkv", "Serie S01E02.mkv")}
<h4>Padronizar extensões</h4>
<p>Extensão → minúsculas. Adicione Extensão → Definir <code>jpg</code> com o filtro <code>*.jpeg</code> para unificar.</p>`,
  },
  {
    id: "shortcuts",
    title: "Atalhos de teclado",
    html: `
<table>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Enter</kbd></td><td>Renomear (funciona até digitando numa regra)</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Z</kbd></td><td>Desfazer a última alteração na lista</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>O</kbd></td><td>Adicionar arquivos</td></tr>
  <tr><td><kbd>F2</kbd></td><td>Editar o novo nome do arquivo selecionado</td></tr>
  <tr><td><kbd>Delete</kbd></td><td>Remover os selecionados da lista</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>A</kbd></td><td>Selecionar todos</td></tr>
  <tr><td><kbd>F1</kbd></td><td>Abrir esta ajuda</td></tr>
  <tr><td><kbd>Esc</kbd></td><td>Fechar janela ou menu</td></tr>
</table>
<p>No macOS, use <kbd>⌘</kbd> no lugar de <kbd>Ctrl</kbd>.</p>`,
  },
  {
    id: "options",
    title: "Opções",
    html: `
<p>A engrenagem abre as opções:</p>
<ul>
  <li><b>Idioma</b> e <b>Tema</b> (claro, escuro ou do sistema).</li>
  <li><b>Lembrar as regras ao fechar o programa</b>.</li>
  <li><b>Incluir arquivos ocultos ao adicionar pastas</b>.</li>
  <li><b>Pedir confirmação antes de renomear</b>.</li>
  <li><b>Desligar as regras depois de renomear</b>.</li>
  <li><b>Avisar sobre caracteres que o Windows não aceita</b>: útil para arquivos que vão para um pendrive ou para o Windows.</li>
</ul>
<p>As opções ficam em <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) ou <code>~/Library/Application Support/renome</code> (macOS). O histórico fica em <code>~/.local/share/renome/history</code> no Linux e na mesma pasta das opções nos outros sistemas.</p>`,
  },
];
