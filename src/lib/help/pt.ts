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
<p>Arquivos ocultos (que começam com ponto, como <code>.config</code>, e, no Windows, os marcados como ocultos ou de sistema, como <code>desktop.ini</code> e <code>Thumbs.db</code>) ficam de fora ao adicionar pastas, a não ser que você ligue a opção correspondente. Uma pasta oculta que você mesmo adiciona entra com tudo o que tem.</p>
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
  <li><b>Remover</b> (ou <kbd>Delete</kbd>) tira os selecionados da lista; <b>Limpar</b> esvazia a lista. Os arquivos no disco não são tocados. O que as buscas acharam para eles também sai, e um arquivo adicionado de novo começa do zero (<kbd>Ctrl</kbd>+<kbd>Z</kbd> traz tudo de volta).</li>
  <li><kbd>Ctrl</kbd>+<kbd>Z</kbd> desfaz a última alteração na lista.</li>
</ul>

<h4>Colunas de informações</h4>
<p>O botão <b>☰</b>, no canto esquerdo do cabeçalho da lista, aparece conforme as regras e liga colunas com o que os arquivos guardam: com a regra de música, <b>Artista</b>, <b>Álbum</b>, <b>Título</b>, <b>Faixa</b>, <b>Ano</b>, <b>Gênero</b>, <b>Duração</b> e <b>Formato</b> (como <code>MP3 · 320 kbps</code> ou <code>FLAC · 24/96</code>); com a de filme ou série, <b>Resolução</b> e <b>Codec</b>; com a regra Inserir usando a data em que a foto foi tirada, <b>Data da foto</b>. Sem nenhuma dessas, o botão não aparece. A escolha fica salva e volta quando a regra volta. Clique no título de uma delas para ordenar a lista: ao ordenar por artista ou ano, cada álbum fica em ordem de disco e faixa. Arquivos sem a informação ficam no fim. O <b>Exportar lista</b> leva também as colunas ligadas.</p>
<p><b>Limitações:</b> as informações são lidas do próprio arquivo, só quando a coluna está ligada; em pastas grandes ou discos lentos, ordenar por uma dessas colunas pela primeira vez leva um tempo, porque cada arquivo precisa ser lido. A resolução e o codec só saem dos formatos de vídeo que o Renome lê (MKV, WebM, MP4, MOV, AVI, MPEG e FLV), e a data da foto só sai de fotos com dados EXIF. Com muitas colunas ligadas numa janela estreita, a lista rola para os lados.</p>`,
  },
  {
    id: "rules",
    title: "Como as regras funcionam",
    html: `
<p>As regras são aplicadas <b>de cima para baixo</b>, cada uma sobre o resultado da anterior. A ordem importa: numerar e depois trocar maiúsculas não dá o mesmo resultado que o contrário.</p>
<ul>
  <li>A <b>caixa de seleção</b> no título liga e desliga a regra sem apagá-la. Desligada, ela fica travada: para mudar as opções ou buscar, ligue-a antes.</li>
  <li><b>↑ ↓</b> mudam a ordem; <b>✕</b> remove a regra; clicar no título recolhe o card.</li>
  <li>Uma regra com erro (expressão inválida, por exemplo) fica com borda vermelha e é ignorada até ser corrigida.</li>
  <li><b>Uma regra de busca por vez</b>: filme, série, música, livro e quadrinho não andam juntas. Com uma delas na lista, as outras ficam desativadas no <b>+ Regra</b> até ela ser removida (desligar não basta). As demais regras (substituir, numerar, inserir…) combinam com qualquer uma. Um conjunto de regras salvo antes, com mais de uma, abre só com a primeira, e um aviso diz quais ficaram de fora. Para uma pasta com filmes e séries misturados, renomeie em duas passadas.</li>
  <li>Uma busca na internet (filme, série, música, livro, quadrinho) pode ser interrompida com <b>Parar</b>, no quadro de progresso. O que já foi encontrado fica, e buscar de novo continua com o resto. A consulta em andamento termina antes, o que pode levar alguns segundos.</li>
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
<p>Nos dois modos "Cada Palavra", algarismos romanos ficam em maiúsculas (<code>Fase III</code>, <code>Parte II</code>) e palavras com maiúsculas no meio ficam como estão (<code>iPhone</code>, <code>McDonald's</code>).</p>
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
  <li><b>Acentos</b>: <code>Canção</code> → <code>Cancao</code>; também letras especiais, como <code>Ørsted</code> → <code>Orsted</code>, <code>Straße</code> → <code>Strasse</code>, <code>Łódź</code> → <code>Lodz</code></li>
  <li><b>Dígitos</b>: <code>foto123</code> → <code>foto</code></li>
  <li><b>Número no início</b>: o número de faixa no começo do nome e o separador depois dele: <code>01 - Música</code>, <code>01. Música</code> e <code>1-05 Música</code> viram <code>Música</code>. Números de até três dígitos: anos (<code>1999 Viagem</code>) e datas (<code>2024-07-15 Praia</code>) ficam, e um nome que é só o número também.</li>
  <li><b>Caracteres</b>: cada caractere digitado é removido onde aparecer. <code>_#</code> remove todos os <code>_</code> e <code>#</code>.</li>
  <li><b>Limitar a</b> N caracteres: corta o nome (sem a extensão) depois de N caracteres, tirando espaços e traços que sobrarem no fim. Útil para som de carro, pen drives e programas antigos que não aceitam nomes longos. Com 0, não há limite. Cuidado: dois nomes longos que só mudam no fim podem ficar iguais, e o conflito aparece na lista.</li>
</ul>`,
  },
  {
    id: "r-convert",
    title: "Converter caracteres",
    sub: true,
    html: `
<p>Troca caracteres por outros, sem apagar nada.</p>
<ul>
  <li><b>Consertar acentos quebrados</b>: nomes criados num sistema e lidos com outra codificação ficam com os acentos trocados por pares de símbolos: <code>CoraÃ§Ã£o</code>, <code>SÃ£o JoÃ£o</code>, <code>donâ€™t</code>. A opção devolve o texto certo: <code>Coração</code>, <code>São João</code>, <code>don’t</code>. Um nome que já está certo fica como está.</li>
  <li><b>Outros alfabetos em letras latinas</b>: cirílico (russo, ucraniano, sérvio…) e grego viram letras latinas, como se pronunciam: <code>Кино</code> → <code>Kino</code>, <code>Щедрик</code> → <code>Shchedrik</code>, <code>Ελλάδα</code> → <code>Ellada</code>. O resto do nome fica igual. Japonês, chinês, árabe e outros alfabetos não são convertidos.</li>
  <li><b>Nome seguro para internet</b>: deixa só letras sem acento, números e pontos, com <code>-</code> ou <code>_</code> entre as palavras: <code>Férias em São Paulo (2024)</code> → <code>Ferias-em-Sao-Paulo-2024</code>. Serve para enviar arquivos a sites, servidores FTP e aparelhos antigos. Também converte cirílico e grego; letras de outros alfabetos saem do nome. Se nada sobrar, o nome fica vazio e a lista mostra o erro.</li>
</ul>
<p><b>Limitações</b>: só conserta o defeito mais comum (texto em UTF-8 lido como Latin-1 ou Windows-1252). Nomes em que os acentos viraram <code>?</code> ou <code>�</code> perderam a letra de vez e não têm conserto. Um nome que mistura letras certas e quebradas na mesma palavra pode ficar como está.</p>
<p>Para consertar nomes tirados das tags, ponha esta regra abaixo da regra Música (tags).</p>`,
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
  <li><b>Pelo conteúdo</b>: olha os primeiros bytes de cada arquivo e dá a extensão do que ele é de verdade. Um <code>.jpg</code> que é PNG vira <code>.png</code>; um <code>.cbz</code> que é RAR vira <code>.cbr</code> (e um <code>.rar</code> com só imagens vira <code>.cbz</code> quando é zip); um arquivo sem extensão ganha uma. Extensões que já servem para aquele conteúdo ficam: um RAW de câmera (<code>.nef</code>, <code>.cr2</code>) é TIFF por dentro e não vira <code>.tif</code>; um <code>.m4b</code> não vira <code>.m4a</code>; MAIÚSCULAS continuam MAIÚSCULAS. Num nome como <code>programa_1.0_x86[0]</code>, o que vem depois do último ponto não é extensão, então a nova é acrescentada no fim. Quando o nome já traz, antes da extensão errada, uma que serve para o conteúdo, a errada só é retirada: <code>filme.nfo.pdf</code> que é texto vira <code>filme.nfo</code>, e <code>foto.png.jpg</code> que é PNG vira <code>foto.png</code>. A extensão de dentro só fica se o arquivo for mesmo daquele tipo: <code>dados.json.pdf</code> só vira <code>dados.json</code> se o conteúdo for JSON válido, e o mesmo vale para XML, HTML, legendas e listas; formatos de texto livre (<code>.txt</code>, <code>.nfo</code>, <code>.csv</code>, <code>.log</code>) aceitam qualquer texto.</li>
</ul>
<p>Texto é tratado à parte: um arquivo de texto só muda quando o nome diz que ele é de um formato que não é texto (<code>.pdf</code>, <code>.jpg</code>, <code>.mp3</code>…). Aí vira <code>.txt</code>, ou <code>.html</code>, <code>.xml</code>, <code>.svg</code>, <code>.json</code>, <code>.srt</code>, <code>.vtt</code>, <code>.ass</code> ou <code>.m3u8</code> quando o começo diz qual é. Um <code>.csv</code>, <code>.md</code>, <code>.log</code> ou um arquivo de texto sem extensão (<code>README</code>) fica como está.</p>
<p><b>Limitações:</b> reconhece imagens, áudio, vídeo, PDF, livros (EPUB, MOBI, FB2, DjVu), arquivos compactados, documentos do Office e do LibreOffice, fontes e texto. Documentos antigos do Office (<code>.doc</code>, <code>.xls</code>), programas e bancos de dados não são reconhecidos e ficam como estão. Entre os textos, só os tipos acima são distinguidos: um CSV com nome <code>.pdf</code> vira <code>.txt</code>. Não converte nada: só corrige o nome.</p>
<p>Pastas não são afetadas por esta regra.</p>`,
  },
  {
    id: "r-movie",
    title: "Filme (TMDB)",
    sub: true,
    html: `
<p>Dá a cada filme o nome oficial do <b>TMDB</b> (themoviedb.org), no formato que o Jellyfin reconhece. Precisa de uma <b>chave da API do TMDB</b>, gratuita, informada nas Opções.</p>
${ex("Nome.do.Filme.2001.1080p.BluRay.x264.mkv", "Nome do Filme (2001) [tmdbid-12345].mkv")}
<div class="notice">
<p><b>Antes de usar: o nome dos arquivos importa</b></p>
<p>O filme é procurado pelo <b>título</b> e pelo <b>ano</b> que estão no nome do arquivo. Nomes cheios de marcas de lançamento (<code>Nome.do.Filme.2001.1080p.BluRay</code>) funcionam bem, porque as marcas são descartadas. Já um nome sem o título (<code>filme1.mkv</code>), só com siglas ou abreviações, ou com o título noutra língua que não a do TMDB, não é achado ou vai para a revisão; sem o ano, filmes com o mesmo nome se confundem.</p>
<p>Se os seus arquivos estão longe disso, ponha acima desta regra, na lista, as regras que acertam o nome (<b>Substituir texto</b>, <b>Remover trecho</b>, <b>Expressão regular</b>): a busca lê o nome como ele sai delas, e a coluna <b>Novo nome</b> mostra o resultado antes de renomear.</p>
</div>
<ol>
  <li>Adicione a regra e clique em <b>Buscar no TMDB</b>. O programa tira o título e o ano do nome de cada arquivo (pontos viram espaços; somem colchetes, parênteses e marcas como 1080p, BluRay, x264) e procura no TMDB.</li>
  <li>Quando o título e o ano batem sem dúvida, o filme é escolhido sozinho (<b>✓</b> ao lado do novo nome). Quando há dúvida (<b>?</b>), abre a janela de revisão: clique no filme certo (ou aperte 1–9). O ano do arquivo pode estar um ano antes ou depois do TMDB; isso é levado em conta.</li>
  <li>Se nenhum servir, corrija o título ou o ano no campo de busca da janela, ou marque <b>Nenhum destes</b> (<b>✕</b>: o arquivo fica como está).</li>
</ol>
<ul>
  <li><b>Modelo</b>: <code>{título}</code>, <code>{original}</code> (título original), <code>{ano}</code> e <code>{id}</code>. O padrão é <code>{título} ({ano}) [tmdbid-{id}]</code>.</li>
  <li>Também, lidos de dentro do vídeo: <code>{resolução}</code> (<code>2160p</code>, <code>1080p</code>, <code>720p</code>, <code>576p</code>, <code>480p</code>; um filme cortado mais largo, como 1920×800, continua <code>1080p</code>) e <code>{codec de vídeo}</code> (<code>H.264</code>, <code>HEVC</code>, <code>AV1</code>, <code>VP9</code>, <code>MPEG-4</code>). São lidos de MKV, WebM, MP4, MOV, M4V, AVI, MPG, VOB e FLV (pelo conteúdo, mesmo com a extensão errada), sem ler o vídeo inteiro, e também de arquivos baixados pela metade; em outros formatos (TS, WMV, RealMedia, ISO) ficam vazios. As legendas recebem os do seu vídeo. Use com um trecho opcional: <code>{título} ({ano})&lt; - {resolução}&gt;</code> dá <code>Filme (2010) - 1080p.mkv</code>, o nome que o Jellyfin usa para guardar várias versões do mesmo filme (com <b>Criar uma pasta para cada filme</b>, as versões ficam juntas numa pasta sem a resolução no nome).</li>
  <li><b>Mais variáveis</b>, do TMDB: <code>{gênero}</code>, <code>{gêneros}</code> (até três), <code>{país}</code>, <code>{idioma}</code> (o original, como <code>en</code>), <code>{duração}</code> (em minutos), <code>{classificação}</code> (a indicativa do país do idioma dos títulos: <code>pt-BR</code> dá a do Brasil), <code>{diretor}</code>, <code>{estúdio}</code>, <code>{coleção}</code> e a data de lançamento. Datas: <code>{data}</code> (<code>2010-05-03</code>), <code>{mês}</code> (<code>05</code>), <code>{dia}</code> e o nome do mês na língua da variável: <code>{nome do mês}</code> dá <code>maio</code>, <code>{month name}</code> dá <code>May</code> e <code>{nombre del mes}</code> dá <code>mayo</code>. Do nome do arquivo: <code>{edição}</code> (Director's Cut, Extended, Unrated, IMAX…), <code>{origem}</code> (BluRay, Remux, WEB-DL, WEBRip, HDTV, DVD…) e <code>{grupo}</code> (o grupo de lançamento: <code>-GRUPO</code> no fim de um nome com origem ou resolução). Limitação: as do TMDB chegam logo depois da busca (uma consulta a mais por filme), e país e estúdio vêm como o TMDB os escreve, em geral em inglês.</li>
  <li><b>O que já está ao lado</b>: se o vídeo tem um <code>.nfo</code> com o mesmo nome (ou a pasta tem um <code>movie.nfo</code>), como os que o Renome, o Jellyfin, o Kodi e o Emby gravam, o ID do TMDB dele diz qual é o filme, sem busca por nome. Com só o ID do IMDb, ou um link do TMDB ou do IMDb no texto (como nas notas de um lançamento), o TMDB é consultado por ele.</li>
  <li><b>Trechos opcionais</b>: um trecho entre <code>&lt; &gt;</code> só aparece quando todas as variáveis dentro dele têm valor (os sinais <code>&lt; &gt;</code> não vão para o nome). <code>&lt;{ano} - &gt;{título}</code> dá <code>2010 - Título</code>, ou só <code>Título</code> quando o ano não é conhecido.</li>
  <li><b>Reserva</b>: <code>{a|b}</code> põe a primeira das variáveis que tiver valor. <code>{título|ano}</code> dá o título, ou o ano quando não há título. Pode ter mais de duas (<code>{a|b|c}</code>) e funciona dentro de um trecho opcional.</li>
  <li><b>Algarismos</b>: <code>{variável:N}</code> escreve os números com N algarismos. <code>{a:3}</code> dá <code>005</code> em vez de <code>05</code>, e <code>{a:1}</code> dá <code>5</code>, sem zeros na frente. Casas decimais (<code>12.1</code>) e o disco antes da faixa (<code>1-05</code>) ficam como estão; texto sem número não muda.</li>
  <li><b>Trocar ":" por</b>: dois-pontos não são aceitos no Windows; por padrão viram <code> - </code>. Barras viram hífens e <code>? * &lt; &gt; |</code> são removidos.</li>
  <li>Só vídeos, legendas e pastas são buscados e renomeados; imagens, <code>.nfo</code> e outros arquivos da lista não mudam de nome, a não ser os que têm o nome do filme ou que vão junto com ele para a pasta nova (veja abaixo). Legendas com o mesmo nome do filme recebem a mesma escolha. Legendas mantêm o idioma e as marcas <b>forced</b> (só as falas em outra língua), <b>sdh</b>/<b>cc</b> (para surdos) e <b>foreign</b> no fim do nome, no formato que o Jellyfin, o Plex e o Kodi reconhecem: <code>Nome do Filme (1999) [tmdbid-12345].pt-BR.forced.srt</code>. Uma marca escrita com traço, sublinhado ou espaço (<code>Filme-forced.srt</code>) vira <code>.forced</code>.</li>
  <li><b>Criar uma pasta para cada filme</b>: o filme e as legendas vão para uma pasta com o novo nome, como o Jellyfin recomenda: <code>Filmes/Nome do Filme (1999) [tmdbid-12345]/Nome do Filme (1999) [tmdbid-12345].mkv</code>. Um filme que já está na pasta certa não é movido de novo. Se ele está numa pasta com o nome dele em outro formato (<code>Nome.do.Filme.1999.1080p</code>), ou numa pasta de qualquer nome que só tenha ele e o que acompanha um filme (legendas, imagens, <code>.nfo</code>, <code>.txt</code>), a pasta nova fica ao lado dela, não dentro (veja <b>Pastas antigas que ficam com arquivos</b>, em Pré-visualização e conflitos). Desfazer devolve os arquivos e apaga a pasta, se ela ficar vazia.</li>
  <li>O que um servidor de mídia deixou na pasta do filme (<code>backdrop.jpg</code>, <code>poster.jpg</code>, <code>movie.nfo</code>, <code>extrafanart/</code>) vai junto para a pasta nova, também ao enviar para outra pasta. Os arquivos com o nome do filme (<code>Nome.nfo</code>, <code>Nome-poster.jpg</code>) ganham o nome novo. Limitação: só vale para arquivos que estão na lista e para a pasta que é só do filme (com o nome dele, ou só com ele e o que acompanha um filme); numa pasta com outras coisas, só os arquivos com o nome do filme vão junto.</li>
  <li>Para tirar os acentos, acrescente a regra <b>Remover caracteres → Acentos</b> depois desta.</li>
</ul>
<h4>Metadados (.nfo e pôster)</h4>
<p>Com <b>Salvar .nfo e pôster ao lado do filme</b>, ao renomear o programa cria, junto de cada vídeo, <code>Nome (ano) [tmdbid-…].nfo</code> e o pôster; opcionalmente também a imagem de fundo (fanart) e o logo do filme. Dentro da pasta do filme, as imagens se chamam <code>poster.jpg</code>, <code>fanart.jpg</code> e <code>logo.png</code>; soltas, levam o nome do filme na frente (<code>…-poster.jpg</code>). Jellyfin, Kodi e Emby leem esses arquivos.</p>
<p>O <code>.nfo</code> traz tudo o que o TMDB tem e o Jellyfin lê: título e título original, ano e data de lançamento, sinopse e frase de efeito, duração, nota e votos, classificação indicativa do país do idioma escolhido (ex.: BR-12), gêneros, palavras-chave (como etiquetas), países, estúdios, coleção (ex.: "Nome da Coleção"), trailer do YouTube, direção, roteiro, elenco com personagens e fotos, produtores, compositor e os IDs do TMDB e do IMDb. O arquivo do filme não é alterado, arquivos que já existem não são substituídos, e <b>desfazer</b> a renomeação apaga o que foi criado.</p>
<p>Filmes que <b>já têm o nome certo</b> também recebem os metadados: o botão Renomear os conta, e a confirmação avisa. Se o nome já traz <code>[tmdbid-…]</code>, o filme é reconhecido direto pelo ID, sem busca. Assim dá para completar uma biblioteca já organizada só com .nfo e imagens.</p>
<h4>Filmes que faltam da coleção</h4>
<p>Com <b>Mostrar os filmes que faltam da coleção</b> (ligada por padrão), quando um filme escolhido faz parte de uma coleção no TMDB (uma trilogia, uma franquia), a lista ganha linhas apagadas para os outros filmes já lançados da coleção que não estão nela, como <code>1999 · Título</code>, com a data de lançamento, na ordem em que saíram. São só um aviso, como os episódios que faltam: não entram em contagens nem em renomeação. <b>Clique no total</b>, na barra de baixo, para salvar a lista num arquivo CSV (coleção, título e lançamento). Para saber a coleção, o programa faz uma consulta a mais ao TMDB por filme e uma por coleção.</p>
<p>Quem decide o que entra numa coleção é o TMDB: às vezes ficam de fora filmes derivados, e às vezes entram filmes que você não consideraria parte dela.</p>
<h4>Limitações</h4>
<ul>
  <li>O filme é procurado pelo título e pelo ano do nome do arquivo. Nomes muito abreviados, só com siglas ou com o título em outro idioma podem não ser achados; nesse caso, corrija o título na revisão.</li>
  <li>Filmes com o mesmo nome (refilmagens, por exemplo) só se distinguem pelo ano; sem o ano no nome, a escolha costuma ir para a revisão.</li>
  <li>Filmes raros, regionais ou muito recentes podem não estar no TMDB, ou estar sem tradução, sinopse ou pôster.</li>
</ul>
<p class="tip">Só o título e o ano procurados (e, para as coleções, o número do filme no TMDB) são enviados ao TMDB, nunca o nome completo do arquivo nem as pastas.</p>`,
  },
  {
    id: "r-series",
    title: "Série (TMDB)",
    sub: true,
    html: `
<p>Dá a cada episódio o nome oficial do <b>TMDB</b>, com temporada, número e título, no formato que o Jellyfin reconhece. Usa a mesma chave da API da regra de filmes.</p>
${ex("Nome.da.Serie.S01E03.720p.WEB-DL.mkv", "Nome da Série (2008) - S01E03 - Título do Episódio.mkv")}
<div class="notice">
<p><b>Antes de usar: o nome dos arquivos importa</b></p>
<p>Cada arquivo é casado com o episódio pelo que o nome dele diz. O nome da série pode vir do campo <b>Série</b>, mas o episódio, não: o nome precisa trazer <b>o número da temporada e do episódio</b> (<code>S01E03</code>, <code>1x03</code>; a temporada também pode vir da pasta) <b>ou o título do episódio</b>. Um nome sem nenhum dos dois (<code>Nome da Série - parte A.mkv</code>, <code>video_0042.mkv</code>) não é achado e vai para a revisão, onde a escolha é feita à mão.</p>
<p>Se os seus arquivos estão longe disso, ponha acima desta regra, na lista, as regras que acertam o nome (<b>Substituir texto</b>, <b>Reorganizar partes</b>, <b>Expressão regular</b>, <b>Numerar</b>): a busca lê o nome como ele sai delas.</p>
</div>
<ol>
  <li>No campo <b>Série</b>, digite o nome, o ID do TMDB ou cole o link da página da série (<code>themoviedb.org/tv/1396-…</code>). Assim, só os arquivos dessa série são buscados; os que trazem no nome outra série (ou o <code>[tmdbid-…]</code> de outra) ficam como estão. <b>Vazio</b>, o programa separa os arquivos pela série que o nome de cada um diz (com o ano, quando os nomes o trazem; pelo número, quando trazem <code>[tmdbid-…]</code>) e busca uma série de cada vez, perguntando quando houver dúvida. Um arquivo sem o nome da série no nome vai com a série da maioria dos arquivos da mesma pasta; numa pasta sem nenhum nome de série, vale o nome da pasta; numa pasta com várias séries e nenhuma maioria, ele fica de fora (o programa avisa quantos).</li>
  <li>Clique em <b>Buscar episódios</b>. Se houver mais de uma série parecida, escolha a certa. O programa baixa a lista completa de episódios e procura o de cada arquivo visível.</li>
  <li>Os episódios encontrados com segurança ganham <b>✓</b>. Os duvidosos (<b>?</b>) e os não encontrados (<b>✕</b>) vão para a janela de revisão.</li>
</ol>
<h4>Como cada episódio é encontrado</h4>
<ul>
  <li><b>Pelo número</b>, em vários formatos: <code>S01E02</code>, <code>s1e2</code>, <code>1x02</code>, <code>T1 E2</code>, <code>Temporada 1 Episódio 2</code>, <code>Ep 02</code> (a temporada vem da pasta, como <code>Season 2</code> ou <code>Temporada 2</code>), numeração corrida (<code>Nome da Série - 125</code>), <code>201</code> (temporada 2, episódio 1) e data de exibição (<code>2024.03.15</code>). Arquivos com dois episódios (<code>S01E01E02</code>, <code>S01E01-02</code>) viram <code>S01E01-E02</code>. Nomes feitos só de blocos entre colchetes (<code>[Grupo][Nome da Série][S03E22][Título][1080p]</code>, também com <code>【】</code>) também são lidos: o bloco com o número, a série antes dele e o título depois.</li>
  <li><b>Pelo título</b>: o texto depois do número é comparado com os títulos de todos os episódios, no seu idioma, em inglês e no idioma original, ignorando acentos, maiúsculas e pequenos erros de grafia; algarismos romanos valem como números (<code>Parte II</code> = <code>Parte 2</code>). Com as temporadas todas numa pasta só, cada uma começando do 1 (<code>02 - Título.mkv</code>), o título escolhe a temporada. Sem número nenhum, o título sozinho basta, se for claro.</li>
  <li><b>Quando o número e o título discordam</b> (o nome diz S02E08, mas o título é o do S01E03), o arquivo fica com <b>?</b>, e a revisão mostra as duas opções, sugerindo a do título.</li>
  <li><b>Arquivos sem o nome da série</b> (<code>06 - Título.mkv</code>) fora da pasta da série só ficam com o episódio do número se o título também combinar (ou se não houver título); senão, vão para a revisão.</li>
</ul>
<h4>Revisão</h4>
<p>Mostra o que foi lido no nome, as sugestões (clique, aperte 1–9, ou <b>Enter</b> para a primeira) e todos os episódios da série por temporada, com um filtro por título ou número (<code>s01e03</code>). <b>Ctrl+clique</b> marca mais de um episódio da mesma temporada, para um arquivo com episódios juntos. Legendas lidas do mesmo jeito recebem a mesma escolha. Só vídeos e legendas são buscados; imagens, <code>.nfo</code> e outros arquivos da lista ficam como estão.</p>
<ul>
  <li><b>Modelo</b>: <code>{série}</code>, <code>{original}</code> (nome original), <code>{ano}</code> (estreia), <code>{temporada}</code>, <code>{episódio}</code> (com dois dígitos), <code>{título}</code> e <code>{id}</code>. O padrão é <code>{série} ({ano}) - S{temporada}E{episódio} - {título}</code>.</li>
  <li>Também, lidos de dentro do vídeo: <code>{resolução}</code> e <code>{codec de vídeo}</code>, como na regra Filme (TMDB). Não entram no nome das pastas da série.</li>
  <li><b>Mais variáveis</b>, do TMDB: <code>{emissora}</code>, <code>{gênero}</code>, <code>{gêneros}</code>, <code>{idioma}</code>, <code>{país}</code>, a data de exibição do episódio e <code>{absoluto}</code>, o número do episódio contado desde o primeiro, sem os especiais (útil para anime: <code>{série} - {absoluto:3}</code>). Datas: <code>{data}</code> (<code>2010-05-03</code>), <code>{mês}</code> (<code>05</code>), <code>{dia}</code> e o nome do mês na língua da variável: <code>{nome do mês}</code> dá <code>maio</code>, <code>{month name}</code> dá <code>May</code> e <code>{nombre del mes}</code> dá <code>mayo</code>. Do nome do arquivo: <code>{edição}</code>, <code>{origem}</code> e <code>{grupo}</code> (também <code>[Grupo]</code> no começo, comum em anime).</li>
  <li><b>O que já está ao lado</b>: um <code>tvshow.nfo</code> na pasta dos episódios (ou na pasta de cima, quando eles estão numa pasta de temporada) diz qual é a série pelo ID do TMDB ou do IMDb, sem busca por nome. Se você digitar a série na regra, vale o que você digitou.</li>
  <li><b>Trechos opcionais</b>: um trecho entre <code>&lt; &gt;</code> só aparece quando todas as variáveis dentro dele têm valor (os sinais <code>&lt; &gt;</code> não vão para o nome). <code>{série} - S{temporada}E{episódio}&lt; - {título}&gt;</code> só põe o traço quando o episódio tem título. Vale também no modelo da pasta da série.</li>
  <li><b>Reserva</b>: <code>{a|b}</code> põe a primeira das variáveis que tiver valor. <code>{título|ano}</code> dá o título, ou o ano quando não há título. Pode ter mais de duas (<code>{a|b|c}</code>) e funciona dentro de um trecho opcional.</li>
  <li><b>Algarismos</b>: <code>{variável:N}</code> escreve os números com N algarismos. <code>{a:3}</code> dá <code>005</code> em vez de <code>05</code>, e <code>{a:1}</code> dá <code>5</code>, sem zeros na frente. Casas decimais (<code>12.1</code>) e o disco antes da faixa (<code>1-05</code>) ficam como estão; texto sem número não muda.</li>
  <li>Episódios sem tradução recebem o título em inglês (ou no idioma original), em vez de "Episódio 4".</li>
  <li>Legendas mantêm o idioma no fim do nome: <code>….pt-BR.srt</code>.</li>
  <li>Os arquivos que não estão na série escolhida ficam como estão. Cada arquivo guarda a sua série; buscar de novo não mexe nos que já estão resolvidos.</li>
</ul>
<h4>Pastas da série e da temporada</h4>
<p>Com <b>Organizar em pastas da série e da temporada</b>, cada episódio vai para a estrutura que o Jellyfin recomenda: <code>Séries/Nome da Série (2008) [tmdbid-12345]/Season 01/…</code>. A pasta da série é criada ao lado da pasta onde a série já está (se os arquivos estão em <code>Nome da Série/Season 1</code>, a nova pasta fica ao lado de <code>Nome da Série</code>, não dentro dela); com os arquivos soltos, dentro da pasta deles. Uma pasta conta como da série quando tem o nome dela ou quando todos os vídeos dentro dela são episódios dessa série, mesmo com outro nome (<code>Nome da Série Completa 1080p</code>); a pasta pessoal, Downloads, Vídeos e o topo de um disco nunca contam. O nome da pasta da série tem o próprio modelo, com as mesmas variáveis (padrão <code>{série} ({ano}) [tmdbid-{id}]</code>; use <code>{original}</code> para o nome original). As pastas antigas que ficarem vazias são apagadas. Se a pasta antiga da série ainda tiver outros arquivos (extras, imagens, um arquivo em conflito), ela fica ao lado da nova e passa a se chamar <code>Nome da Série (2008) [tmdbid-12345] (arquivos adicionais)</code>, para as duas aparecerem juntas; antes de renomear, a barra de baixo mostra <b>pasta(s) com sobras</b>, e a confirmação diz qual pasta e quantos arquivos ficam. Depois de renomear, a pasta antiga pode sumir (<code>Desenhos/Nome da Série</code>), e os arquivos aparecem na nova, ao lado dela (<code>Desenhos/Nome da Série (2008) [tmdbid-12345]/Season 01/…</code>): <b>nada foi apagado, só movido</b>. <b>Desfazer</b> devolve tudo ao lugar, recriando as pastas antigas.</p>
<h4>Metadados (.nfo e imagens)</h4>
<p>Com <b>Salvar .nfo e imagens para o Jellyfin</b>, ao renomear o programa cria:</p>
<ul>
  <li>na <b>pasta da série</b>: <code>tvshow.nfo</code> (título, título original, ano, sinopse, situação, duração, nota, classificação indicativa, gêneros, palavras-chave, emissoras e estúdios, criadores, trailer, elenco com personagens e os IDs do TMDB, IMDb e TVDB), <code>poster.jpg</code> e o pôster de cada temporada (<code>season01-poster.jpg</code>); opcionalmente <code>fanart.jpg</code> e <code>logo.png</code>;</li>
  <li>na <b>pasta da temporada</b>: <code>season.nfo</code>, com o nome e a sinopse da temporada;</li>
  <li>ao lado de <b>cada episódio</b>: <code>….nfo</code> (título, temporada e número, data de exibição, sinopse, duração, nota, direção, roteiro e atores convidados) e a miniatura <code>…-thumb.jpg</code>.</li>
</ul>
<p>Os arquivos da série e da temporada só são criados quando os episódios estão numa pasta da série (com o nome dela, ou com <code>[tmdbid-…]</code>), seja a de antes, seja a criada pela opção de pastas. Episódios que já têm o nome certo também recebem os metadados. Arquivos que já existem não são substituídos, e desfazer apaga o que foi criado.</p>
<h4>Episódios que faltam</h4>
<p>Com <b>Mostrar os episódios que faltam</b> (ligada por padrão), depois da busca a lista ganha linhas apagadas para os episódios já exibidos que nenhum vídeo da lista tem, por exemplo <code>S01E16 · Título</code>, com a data de exibição. Elas ficam no lugar do episódio, entre os vizinhos, e a barra de baixo mostra quantos faltam. São só um aviso: não podem ser selecionadas e não entram em contagens, renomeação nem na exportação da lista. <b>Clique nesse total</b> para salvar os episódios que faltam num arquivo CSV (série, temporada, episódio, código, título e data de exibição), para abrir numa planilha. A janela abre na pasta da série, com o nome dela no arquivo; dá para escolher outro lugar.</p>
<p>Só entram as temporadas de que a lista tem pelo menos um vídeo; temporadas inteiras que você não tem não aparecem. Episódios ainda não exibidos ou sem data também não. Um episódio que só tem a legenda conta como faltando.</p>
<h4>Limitações</h4>
<ul>
  <li>Uma série só é reconhecida no nome do arquivo pelos nomes que tem no TMDB (em qualquer idioma ou país) ou pelo que você digitou. Um apelido ou uma sigla que o TMDB não conhece faz o arquivo ser tratado como de outra série: digite a série, ou use esse nome no campo.</li>
  <li>A divisão em temporadas é a do TMDB. Animes e séries com numeração corrida às vezes estão divididos de outro jeito lá, e os números do arquivo não batem; confira na revisão.</li>
  <li>Os especiais (temporada 0) costumam ter numeração diferente da usada nos lançamentos e quase sempre precisam de revisão.</li>
  <li>Um arquivo com vários episódios só é reconhecido quando eles são da mesma temporada.</li>
  <li>Os episódios que faltam seguem a numeração do TMDB: se seus arquivos usam outra (a do DVD, ou a numeração corrida de animes), a lista do que falta pode sair errada. Episódios escondidos pelo filtro também contam como faltando.</li>
</ul>
<p class="tip">Só o nome da série é enviado ao TMDB, nunca o nome dos arquivos nem as pastas.</p>`,
  },
  {
    id: "r-music",
    title: "Música (tags)",
    sub: true,
    html: `
<p>Dá a cada música o nome que vem das <b>tags</b> gravadas no arquivo (ID3 do MP3, Vorbis do FLAC e do OGG, MP4 do M4A e outras), sem precisar de internet.</p>
${ex("01.mp3", "01 - Título da Música.mp3")}
<ul>
  <li><b>Modelo</b>: <code>{faixa}</code>, <code>{título}</code>, <code>{artista}</code>, <code>{artista do álbum}</code>, <code>{álbum}</code>, <code>{ano}</code>, <code>{disco}</code>, <code>{gênero}</code> e <code>{compositor}</code>. O padrão é <code>{faixa} - {título}</code>. A faixa tem dois dígitos (três em álbuns com mais de 99 faixas). <code>{disco}</code> só tem valor em álbuns de mais de um disco: num álbum de um disco só, fica vazio.</li>
  <li>Também: <code>{total de faixas}</code>, <code>{total de discos}</code>, <code>{ano original}</code> (o do primeiro lançamento do álbum; sem ele, o <code>{ano}</code>), <code>{gravadora}</code>, <code>{catálogo}</code> (o número de catálogo), <code>{artista invertido}</code> e <code>{artista do álbum invertido}</code> (como o artista é ordenado: <i>Beatles, The</i>; sem essa tag, o próprio nome) e <code>{inicial}</code> (a primeira letra do artista do álbum invertido, para pastas como <code>{inicial}/{artista do álbum invertido}</code>). Esses vêm das tags ou, quando o álbum é encontrado no MusicBrainz, do lançamento. Do próprio arquivo: <code>{formato}</code> (<code>FLAC</code>, <code>MP3</code>…), <code>{taxa de bits}</code> (em kbit/s), <code>{amostragem}</code> (em kHz: <code>44.1</code>, <code>96</code>) e <code>{bits}</code> (<code>16</code>, <code>24</code>; formatos com perda, como MP3, não têm). Exemplo: <code>{álbum} &lt;[{formato} {bits}-{amostragem}]&gt;</code> dá <code>Álbum [FLAC 24-96]</code>.</li>
  <li><b>Data de lançamento</b>, das tags (ou do MusicBrainz, ao identificar o álbum). Datas: <code>{data}</code> (<code>2010-05-03</code>), <code>{mês}</code> (<code>05</code>), <code>{dia}</code> e o nome do mês na língua da variável: <code>{nome do mês}</code> dá <code>maio</code>, <code>{month name}</code> dá <code>May</code> e <code>{nombre del mes}</code> dá <code>mayo</code>. Limitação: muitos arquivos só têm o ano; aí mês e dia ficam vazios.</li>
  <li><b>Trechos opcionais</b>: um trecho entre <code>&lt; &gt;</code> só aparece quando todas as variáveis dentro dele têm valor (os sinais <code>&lt; &gt;</code> não vão para o nome). No modelo das pastas, <code>{artista do álbum}/{álbum}/&lt;CD{disco}&gt;</code> cria a pasta <code>CD1</code>, <code>CD2</code>… só em álbuns de mais de um disco.</li>
  <li><b>Reserva</b>: <code>{a|b}</code> põe a primeira das variáveis que tiver valor. <code>{título|ano}</code> dá o título, ou o ano quando não há título. Pode ter mais de duas (<code>{a|b|c}</code>) e funciona dentro de um trecho opcional.</li>
  <li><b>Algarismos</b>: <code>{variável:N}</code> escreve os números com N algarismos. <code>{a:3}</code> dá <code>005</code> em vez de <code>05</code>, e <code>{a:1}</code> dá <code>5</code>, sem zeros na frente. Casas decimais (<code>12.1</code>) e o disco antes da faixa (<code>1-05</code>) ficam como estão; texto sem número não muda.</li>
  <li>Sem número de faixa nas tags, fica o número com que o nome do arquivo começa (<code>07. Título.flac</code>), para a ordem não se perder. Um álbum fica com um ano só, o mais comum entre as faixas: uma faixa bônus de um relançamento não ganha uma pasta à parte.</li>
  <li>Tags vazias não deixam sobras: com <code>{faixa} - {artista} - {título}</code>, uma música sem artista vira <code>01 - Título</code>.</li>
  <li><b>Número do disco na faixa</b>: em álbuns com mais de um disco, a faixa vira <code>1-01</code>, <code>2-01</code>…, para os discos não se misturarem.</li>
  <li><b>O álbum como um todo</b>: as faixas de um mesmo álbum (mesma pasta, ou discos <code>CD 01</code>/<code>CD 02</code>, e mesmo nome de álbum) combinam os dados do álbum. Uma faixa sem ano recebe o das outras. Sem "artista do álbum" nas tags, vale o artista da maioria das faixas (desconsiderando participações, como <code>feat.</code>); sem maioria, o álbum é uma coletânea: <code>Various Artists</code>, como o Jellyfin reconhece. Assim, as faixas de um álbum nunca se espalham em pastas diferentes.</li>
  <li><b>Faixas bônus</b>: numa pasta em que a maioria das músicas é de um álbum, as faixas com outro álbum nas tags (um single, uma trilha sonora) e numeradas depois das do álbum (<code>15</code>, <code>16</code> depois de <code>01</code>–<code>14</code>) ficam com esse álbum.</li>
  <li><b>Singles</b>: uma música com o álbum <code>Single</code> nas tags tem o próprio título como álbum.</li>
  <li><b>Coletâneas</b>: num álbum em que a maioria das faixas é de outros artistas que não o do álbum, o artista entra no nome antes do título quando o modelo não o tem: <code>03 - Artista da Faixa - Título</code>.</li>
  <li>Se o nome do álbum já traz um ano (<code>Programa 12 (2025-05-04)</code>), o <code>({ano})</code> do modelo é omitido.</li>
  <li>Músicas <b>sem título nas tags</b> ficam como estão e ganham <b>✕</b> (a não ser que a busca as identifique pelo som, veja abaixo); o card mostra quantas são. Com <b>Completar pelo nome do arquivo</b>, o programa lê <code>01 - Artista - Título</code>, <code>01. Título</code> ou <code>1-05 Título</code> do nome, e o álbum e o ano da pasta (<code>Artista - Álbum (1975)</code>, ou só <code>Álbum</code>; uma pasta <code>CD2</code> dá o disco). As tags que existem têm preferência.</li>
</ul>
<h4>Pastas de artista e álbum</h4>
<p>Com <b>Organizar em pastas de artista e álbum</b>, cada música vai para <code>{artista do álbum}/{álbum} ({ano})</code> (o modelo das pastas pode ser trocado; cada <code>/</code> é um nível). Assim, <code>Música/Artista/Album/CD1/01.mp3</code> vira <code>Música/Artista/Álbum (1975)/01 - Título da Música.mp3</code>: as pastas que já levam o nome do artista, do álbum ou do disco (ou que estão dentro da pasta do artista) são substituídas, não ganham pastas novas dentro. Arquivos soltos ganham as pastas dentro da pasta onde estão, a não ser que ela só tenha músicas de um álbum: aí ela conta como a pasta do álbum, qualquer que seja o nome (<code>Músicas/Pasta nova/01.flac</code> vira <code>Músicas/Artista/Álbum (1975)/01 - Título da Música.flac</code>), e é apagada se ficar vazia. A pasta pessoal, Música, Downloads e as outras pastas do sistema nunca contam. Pequenas diferenças de grafia entre a pasta e a tag (<code>Сказка об</code> e <code>Сказка о</code>) não atrapalham. Uma pasta que já reúne álbuns com o nome dela (<code>Programa (1000 - ..)</code> para os álbuns "Programa 1037 - …") é tratada como coleção: os álbuns ficam direto nela, sem pastas de artista no meio.</p>
<div class="notice">
<p><b>Pastas de músicas avulsas ficam juntas</b></p>
<p>Uma pasta em que quase todas as músicas são de álbuns diferentes, com uma ou duas faixas de cada (como uma pasta <code>Avulsas</code>, <code>Misc</code> ou <code>Singles</code>), é tratada como uma coletânea que você montou. As músicas dela são renomeadas normalmente, mas <b>ficam nela</b>: não vão cada uma para a pasta do seu álbum. O álbum de cada uma continua nas tags.</p>
<p>A regra exata: pelo menos 3 músicas, de pelo menos 3 álbuns, e pelo menos 4 em cada 5 desses álbuns com uma ou duas faixas na pasta. Uma pasta de álbum de verdade, com várias faixas do mesmo disco, é organizada como sempre. Se quiser separar as avulsas mesmo assim, mova-as à mão para as pastas que preferir.</p>
</div>
<p><b>Cópias</b>: se duas cópias idênticas de um álbum (encontradas pelo programa, como em <b>Cópias idênticas</b>) iriam para o mesmo lugar, a primeira segue e a outra fica onde está, marcada como cópia.</p>
<ul>
  <li>Letras (<code>.lrc</code>) com o mesmo nome de uma música recebem o novo nome dela e vão junto.</li>
  <li>Capas, <code>.cue</code>, <code>.log</code> e outros arquivos da pasta do álbum vão junto com a maioria das músicas da pasta (músicas de outros álbuns que não sejam faixas bônus seguem o álbum delas). Subpastas sem músicas (<code>Artwork</code>, <code>Scans</code>) vão junto, com o mesmo nome; arquivos da pasta acima de <code>CD 01</code>/<code>CD 02</code> também. Se dois desses arquivos tiverem o mesmo nome (um <code>log.txt</code> em cada disco), ganham o nome da pasta de origem: <code>log (CD 01).txt</code>.</li>
  <li>Nomes de artista ou álbum que só diferem em maiúsculas (<code>nome do artista</code> e <code>Nome do Artista</code>, de tags de álbuns diferentes) viram uma pasta só: a que já existe no disco ou a grafia mais usada.</li>
  <li>Pastas antigas que ficarem vazias são apagadas; desfazer devolve tudo ao lugar.</li>
  <li><b>Criar uma playlist por álbum</b>: depois de renomear, grava <code>&lt;nome da pasta&gt;.m3u8</code> na pasta de cada álbum, com as músicas (de todos os discos) na ordem dos discos e faixas. Vale também para os álbuns que já estão com os nomes certos (eles contam no botão <b>Renomear</b>). Não é criada se o álbum já tiver uma playlist (<code>.m3u</code> ou <code>.m3u8</code>): essa é atualizada com os novos nomes, e o resumo do fim avisa. Um <code>.cue</code> não impede, porque descreve o disco, e não serve de playlist para a maioria dos players. Também não é criada para uma música sozinha. Desfazer apaga a playlist criada.</li>
  <li>Arquivos <code>.cue</code> e listas <code>.m3u</code> passam a citar os novos nomes das músicas (veja <b>Renomear e desfazer</b>).</li>
</ul>
<h4>Corrigir pelo MusicBrainz</h4>
<p>Para tags erradas ou incompletas, <b>Buscar no MusicBrainz</b> identifica cada álbum no <b>MusicBrainz</b> (musicbrainz.org), um banco de dados aberto de música. Não precisa de chave.</p>
<ul>
  <li>As músicas visíveis são agrupadas por álbum. Se as tags já trazem o ID do MusicBrainz, o álbum é reconhecido direto (quando todas as músicas estão nele; senão, vai para a conferência); senão, o programa busca pelo artista e pelo nome do álbum.</li>
  <li>Um mesmo álbum costuma ter várias edições (original, remasterizada, de luxo, de outro país). O programa confere o <b>número de faixas</b>, o <b>título</b> e a <b>duração de cada uma</b> (uma faixa com o mesmo número mas outro título e outra duração não conta): se a melhor edição encontrada bate com as músicas, ela é escolhida sozinha (<b>✓</b>). Senão, abre a revisão, com capa, data, país, formato, gravadora e número de faixas (em verde quando é igual ao da pasta), para você escolher, ou <b>Nenhum</b> para manter as tags.</li>
  <li>Com o álbum escolhido, os nomes e as pastas usam os dados do MusicBrainz: título de cada faixa, artista, artista do álbum, nome do álbum e ano, com a grafia oficial. Cada música é casada com a sua faixa pelo disco e número (ou pelo título).</li>
  <li><b>Salvar a capa do álbum</b>: grava <code>cover.jpg</code> (do Cover Art Archive) na pasta do álbum, se ela ainda não tiver capa. Se a edição escolhida não tiver imagem (uma fita cassete, por exemplo), usa a capa do álbum. Desfazer apaga a capa criada.</li>
  <li><b>Gravar as tags nos arquivos</b>: depois de renomear, grava nas músicas identificadas tudo o que o MusicBrainz sabe delas: título, artistas (também cada um em separado), artista do álbum, álbum, data e data original, faixa e disco (com os totais), gravadora, número de catálogo, código de barras, país, tipo de lançamento, mídia, ISRC, gêneros, idioma, compositor, letrista, produtor, arranjador, mixagem e os IDs do MusicBrainz, que o Jellyfin, o Navidrome e o Plex usam. Para isso, cada álbum é consultado mais uma vez (uns 2 segundos). Vale também para as músicas que já estão com o nome certo (elas contam no botão <b>Renomear</b>). Uma música que já tem tudo isso não é regravada. Funciona com MP3 (gravado em ID3v2.3), FLAC, OGG, Opus e M4A.</li>
  <li><b>Apagar as outras tags</b>: tira o que não vem do MusicBrainz (comentários, endereços de sites, dados de quem ripou, "codificado por"), e no MP3 também as tags secundárias (ID3v1 e APE). Ficam as letras, o ReplayGain, o BPM, a folha de faixas embutida (CUESHEET) e o gênero e o compositor quando o MusicBrainz não os tiver. Desmarcada, as outras tags ficam como estão.</li>
  <li><b>Incluir a capa nos arquivos</b>: tira as imagens que já estão dentro de cada música e coloca a capa do álbum (do Cover Art Archive ou, se não houver, a <code>cover.jpg</code> da pasta). Em <b>Tamanho da capa</b>, escolha 500 ou 250 px para aparelhos de som de carro e tocadores antigos que travam ou não mostram capas grandes; as músicas também ficam menores. A <code>cover.jpg</code> salva na pasta continua grande. Limitação: a capa tirada da pasta (quando o Cover Art Archive não tem) entra no tamanho que tiver, sem ser reduzida.</li>
  <li>A gravação é feita numa cópia, que só toma o lugar do arquivo no fim; uma queda no meio não estraga a música. Se o programa for fechado ou o computador desligar durante a gravação, na próxima vez que o Renome abrir as músicas já gravadas vão para o <b>Histórico</b>, para desfazer, e a cópia que estava sendo gravada é apagada. Desfazer devolve as tags e as imagens antigas (no MP3 e no M4A, campos raros que a limpeza tirou e que não são texto, como dados binários de outros programas, e as tags ID3v1 e APE não voltam). A data de modificação muda, para que os players releiam as tags. Como o conteúdo muda, um torrent não reconhece mais esses arquivos.</li>
  <li>As consultas são espaçadas (uma a cada 2 segundos), como o MusicBrainz pede; cada álbum leva uns 4 segundos.</li>
</ul>
<h4>Discogs (opcional)</h4>
<p>Muitos lançamentos regionais, independentes e em vinil estão no <b>Discogs</b> (discogs.com) e não no MusicBrainz. Para usá-lo, crie uma conta gratuita no Discogs, gere um <b>token pessoal</b> em Configurações → Desenvolvedores e cole-o em <b>Opções → Discogs</b>.</p>
<ul>
  <li>Com o token, quando o MusicBrainz não acha nada para um álbum, o programa procura no Discogs pelo artista e pelo nome do álbum. A edição é conferida do mesmo jeito (número de faixas, títulos e durações): se bate, é escolhida sozinha; senão, vai para a revisão. Os candidatos do Discogs têm a etiqueta <b>Discogs</b>.</li>
  <li>Na revisão, ao lado de <b>Buscar</b>, escolha <b>MusicBrainz</b> ou <b>Discogs</b> para procurar na outra fonte, mesmo que a primeira tenha achado algo.</li>
  <li>Os nomes, as pastas, a capa (<code>cover.jpg</code>), as faixas que faltam e a gravação das tags funcionam igual, com os dados do Discogs. Os discos saem da posição das faixas (<code>1-05</code>, <code>CD2-3</code>); lados de vinil (<code>A1</code>, <code>B2</code>) são numerados como um disco só. As partes de uma obra (<code>Sonata: I. Allegro</code>) contam como faixas.</li>
  <li>Ao Discogs vão só o artista e o nome do álbum, com o seu token. O token fica salvo só neste computador, no cofre de senhas do sistema.</li>
</ul>
<p><b>Limitações do Discogs:</b> só é consultado quando o MusicBrainz não acha nada (ou quando você escolhe na revisão); a identificação pelo som não usa o Discogs. As tags gravadas a partir do Discogs têm menos campos: título, artistas, álbum, datas, faixa, disco, gravadora, número de catálogo, código de barras e gêneros (os estilos do Discogs entram primeiro); não há IDs, compositores, produtores nem ISRC, e por isso uma próxima busca procura de novo. A capa é a do Discogs, em geral com uns 600 pixels: o <b>Tamanho da capa</b> pode reduzi-la (500 ou 250), mas a opção Grande fica com os 600. O país e o ano mostrados na revisão são os da busca; o número de faixas só aparece depois de abrir a edição. O Discogs aceita cerca de 60 consultas por minuto; cada álbum usa 2 ou 3.</p>
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
<h4>Faixas que faltam</h4>
<p>Com <b>Mostrar as faixas que faltam</b> (ligada por padrão), nos álbuns identificados no MusicBrainz a lista ganha linhas apagadas para as faixas da edição escolhida que nenhuma música da lista tem, como <code>05 · Título</code> (<code>2-05</code> em álbuns com mais de um disco), no lugar delas, entre as vizinhas. São só um aviso: não entram em contagens nem em renomeação. <b>Clique no total</b>, na barra de baixo, para salvar a lista num arquivo CSV (álbum, disco, faixa e título). Não há consulta a mais: a lista de faixas é a que já foi usada para identificar o álbum.</p>
<p>A lista é a da <b>edição escolhida</b>: uma edição com faixas bônus mostra as bônus que você não tem; outra edição do mesmo álbum pode não ter. Na revisão dá para escolher a edição que corresponde à sua. Uma música que não casou com nenhuma faixa da edição faz a faixa dela aparecer como faltando.</p>
<h4>Limitações</h4>
<ul>
  <li>Sem a busca no MusicBrainz, o nome depende só das tags: tags erradas geram nomes errados.</li>
  <li>Lançamentos regionais, independentes, piratas ou muito recentes podem não estar no MusicBrainz; aí valem as tags, a não ser que o Discogs (com o seu token) os tenha.</li>
  <li>Pelo som, gravações muito parecidas (uma remasterização, uma regravação do próprio artista) podem ser confundidas. Quando o álbum fica em dúvida, ele vai para a revisão.</li>
  <li>Músicas em Opus, WMA e outros formatos não listados acima não são ouvidas.</li>
  <li>No MP3 e no M4A, desfazer a gravação das tags não traz de volta os campos raros que não são texto nem as tags ID3v1 e APE.</li>
  <li>No MP3, vários valores de um campo (gêneros, compositores…) ficam num só (<code>Rock; Pop</code>), porque o formato usado para que o Windows leia as tags (ID3v2.3) só guarda um valor por campo.</li>
</ul>
<p class="tip">Sem <b>Gravar as tags nos arquivos</b>, o Renome só lê as tags; o conteúdo das músicas não é alterado. Ao MusicBrainz vão só o artista e o nome do álbum.</p>`,
  },
  {
    id: "r-book",
    title: "Livro (Open Library)",
    sub: true,
    html: `
<p>Dá a cada livro o nome que vem do <b>Open Library</b> (openlibrary.org), um catálogo aberto de livros mantido pelo Internet Archive. Quando o Open Library não conhece o ISBN, ele é procurado na <b>BrasilAPI</b> (brasilapi.com.br), que reúne os registros das editoras brasileiras na CBL. Nenhum dos dois precisa de chave.</p>
${ex("titulo_do_livro_autor.pdf", "Nome do Autor - Título do Livro (1937).pdf")}
<ol>
  <li>Adicione a regra e clique em <b>Buscar no Open Library</b>. Para cada livro (EPUB, MOBI, AZW3, PDF, FB2 e DjVu), o programa lê os dados gravados no próprio arquivo: título, autor, ISBN, editora e série. O que faltar vem do nome do arquivo.</li>
  <li>Com o <b>ISBN</b>, o livro é achado direto, na edição exata (<b>✓</b>), no Open Library ou na BrasilAPI. Sem ele, o programa busca pelo título e pelo autor: se batem sem dúvida, o livro é escolhido sozinho; senão (<b>?</b>), abre a revisão, com capa, autores, título original e número de edições, para você escolher (clique, ou aperte 1–9).</li>
  <li>Se nenhum servir, corrija o título ou o autor na janela e busque de novo. Dá para buscar só pelo autor, com o título vazio, e ver os livros dele. Ou marque <b>Nenhum destes</b> (<b>✕</b>: o arquivo fica como está).</li>
</ol>
<h4>De onde vêm os dados</h4>
<ul>
  <li><b>EPUB</b>: título, autores, ISBN, editora, idioma e série (dos dados do EPUB 2 ou 3). Sem ISBN nos dados, o programa o procura no texto das primeiras e das últimas páginas, onde fica a ficha catalográfica.</li>
  <li><b>MOBI e AZW3</b> (Kindle): título, autor, ISBN, editora e data, do cabeçalho do arquivo.</li>
  <li><b>PDF</b>: título e autor dos dados do documento, e o ISBN procurado no texto das primeiras e das últimas páginas. Títulos que os programas põem sozinhos (<code>Microsoft Word - final.docx</code>, <code>Sem título</code>) são ignorados.</li>
  <li><b>FB2</b>: título, autores, série, editora, ano e ISBN.</li>
  <li><b>DjVu</b>: só o nome do arquivo.</li>
  <li><b>Nome do arquivo</b>: entende <code>Autor - Título</code> e <code>Título - Autor</code> (a busca descobre qual é qual), <code>Título by Autor</code>, o ano entre parênteses (<code>(1899)</code>, <code>(2015, Editora)</code>), o número na série (<code>Autor - 03 - …</code>) e o ISBN. Marcas como <code>z-lib.org</code>, <code>libgen</code>, <code>epub</code> e <code>retail</code> são ignoradas, e <code>_</code> vira espaço.</li>
</ul>
<ul>
  <li><b>Modelo</b>: <code>{autor}</code> (o primeiro), <code>{autores}</code> (todos, separados por &amp;), <code>{autor invertido}</code> (pelo sobrenome: <i>Sobrenome, Nome</i>), <code>{inicial}</code> (a primeira letra do sobrenome), <code>{título}</code>, <code>{título original}</code>, <code>{ano}</code> (da primeira publicação da obra), <code>{ano da edição}</code>, <code>{editora}</code>, <code>{isbn}</code>, <code>{série}</code>, <code>{volume}</code> (o número na série, com dois dígitos: <code>03</code>, para os livros ficarem em ordem) e <code>{id}</code> (do Open Library). O padrão é <code>{autor} - {título} ({ano})</code>. Valores que faltam não deixam sobras: sem série, <code>{série} {volume} - {título}</code> vira só o título.</li>
  <li><b>Mais variáveis</b>: <code>{idioma}</code> (<code>pt</code>, <code>en</code>) e <code>{páginas}</code> (do Open Library: as da edição achada pelo ISBN, senão o número mais comum entre as edições). Limitação: o tradutor não está disponível, porque o Open Library não informa.</li>
  <li><b>O que já está ao lado</b>: um <code>.opf</code> com o mesmo nome do livro, ou o <code>metadata.opf</code> de uma pasta que só tem esse livro (como os que o Renome e os gerenciadores de biblioteca gravam), vale mais que os dados de dentro do arquivo: título, autores, ISBN, ano, editora, idioma e série vêm dele.</li>
  <li><b>Trechos opcionais</b>: um trecho entre <code>&lt; &gt;</code> só aparece quando todas as variáveis dentro dele têm valor (os sinais <code>&lt; &gt;</code> não vão para o nome). <code>{autor} - &lt;{série} {volume} - &gt;{título}</code> dá <code>Autor - Série 03 - Título</code> para livros de uma série e <code>Autor - Título</code> para os outros. Vale também no modelo das pastas.</li>
  <li><b>Reserva</b>: <code>{a|b}</code> põe a primeira das variáveis que tiver valor. <code>{título|ano}</code> dá o título, ou o ano quando não há título. Pode ter mais de duas (<code>{a|b|c}</code>) e funciona dentro de um trecho opcional.</li>
  <li><b>Algarismos</b>: <code>{variável:N}</code> escreve os números com N algarismos. <code>{a:3}</code> dá <code>005</code> em vez de <code>05</code>, e <code>{a:1}</code> dá <code>5</code>, sem zeros na frente. Casas decimais (<code>12.1</code>) e o disco antes da faixa (<code>1-05</code>) ficam como estão; texto sem número não muda.</li>
  <li><b>Trocar título e autor</b>: o botão ⇄ na revisão inverte os dois campos, para nomes lidos ao contrário.</li>
  <li>O <b>título</b> é o da edição no idioma escolhido nas Opções (<b>Idioma dos títulos</b>). Quando o arquivo traz outro título, de outra tradução, fica o do arquivo. O título original aparece na revisão e em <code>{título original}</code>.</li>
  <li>O mesmo livro em vários formatos (<code>.epub</code>, <code>.pdf</code>, <code>.mobi</code>) com os mesmos dados faz uma busca só e recebe a mesma escolha.</li>
</ul>
<h4>Pastas, capa e dados</h4>
<ul>
  <li><b>Organizar em pastas de autor e livro</b>: cada livro vai para <code>{autor}/{título}</code> (o modelo das pastas pode ser trocado; cada <code>/</code> é um nível), como o Audiobookshelf espera. Em bibliotecas grandes, <code>{inicial}/{autor invertido}/{título}</code> dá <code>S/Sobrenome, Nome/Título do Livro</code>. Um ponto no fim do nome de uma pasta (<i>Sobrenome, C. S.</i>) sai, porque o Windows não aceita pastas terminadas em ponto. Pastas que já têm o nome do autor ou do livro são aproveitadas, sem ganhar pastas novas dentro. Uma pasta que só tem um livro (em um ou mais formatos) conta como a pasta dele, qualquer que seja o nome: as pastas novas ficam ao lado dela, não dentro. Pastas que ficarem vazias são apagadas; desfazer devolve tudo ao lugar.</li>
  <li><b>Salvar a capa</b>: grava a capa do Open Library como <code>cover.jpg</code> na pasta do livro; sem a opção de pastas, com o nome do arquivo do livro (<code>….jpg</code>).</li>
  <li><b>Salvar os dados do livro</b>: grava <code>metadata.opf</code> (ou <code>….opf</code>, sem a opção de pastas), no formato OPF, com título, autores, sinopse, assuntos, editora, ano, idioma, ISBN e série. O Audiobookshelf e os gerenciadores de e-books leem esse arquivo.</li>
  <li>Livros que já têm o nome certo também recebem a capa e os dados (eles contam no botão <b>Renomear</b>). Arquivos que já existem não são substituídos, e desfazer apaga o que foi criado. Sem a opção abaixo, o arquivo do livro nunca é alterado.</li>
  <li><b>Gravar os dados dentro do EPUB</b>: título, autores, ano, editora, idioma, ISBN e série vão para os dados de dentro do próprio EPUB (o arquivo OPF que vem nele), que os leitores de e-book e os gerenciadores mostram. Só os dados que mudam são trocados; o resto fica como estava: sinopse, assuntos, ilustradores, tradutores, a capa e o identificador próprio do livro (que leitores como o Kobo usam para reconhecê-lo). Um ISBN novo entra ao lado desse identificador, sem tomar o lugar dele. A série é gravada nos dois jeitos mais lidos (o antigo e o do EPUB 3, nos EPUB 3). O texto, as imagens e o resto do livro são copiados como estão. Livros que já têm o nome certo também recebem os dados, e os que já têm os mesmos dados são pulados.</li>
  <li>A gravação é feita numa cópia, que só toma o lugar do livro no fim; se o programa for fechado no meio, o que já foi gravado vai para o <b>Histórico</b>. Desfazer devolve os dados antigos exatamente como eram. Como o conteúdo muda, um torrent não reconhece mais esses arquivos.</li>
</ul>
<h4>Volumes que faltam</h4>
<p>Com <b>Mostrar os volumes que faltam da série</b> (ligada por padrão), depois da busca, nos livros de uma série a lista ganha linhas apagadas para os volumes que faltam <b>entre os seus</b>, do 1 até o maior número que você tem, como <code>3</code>, no lugar deles. São só um aviso: não entram em contagens nem em renomeação. <b>Clique no total</b>, na barra de baixo, para salvar a lista num arquivo CSV (série e volume).</p>
<p>Nenhum catálogo tem uma lista confiável dos livros de cada série, por isso o programa só vê os buracos entre os seus arquivos: o que vem depois do seu último volume não aparece. A série vem dos dados do livro (EPUB, FB2); o número, deles ou do nome do arquivo (<code>Autor - 03 - Título</code>). Livros sem série nos dados não entram. Volumes como <code>2.5</code> (contos entre dois livros) não preenchem nem abrem buracos.</p>
<h4>Limitações</h4>
<ul>
  <li><b>PDF</b>: muitos PDFs têm os dados do documento vazios ou errados (o nome do arquivo do Word, o nome do computador de quem o fez). Um PDF <b>escaneado</b> é só uma sequência de imagens, sem texto onde procurar o ISBN. Nesses casos resta o nome do arquivo: um nome claro, como <code>Autor - Título</code>, ajuda muito; um nome confuso leva à revisão. Vale acertar o nome desses arquivos com as outras regras, postas acima desta na lista: a busca lê o nome como ele sai delas.</li>
  <li><b>Sem ISBN</b>, o livro é achado pelo título e pelo autor, mas não se sabe qual é a edição: o nome usa a editora e o ISBN do próprio arquivo, quando ele os tem, e <code>{ano da edição}</code> fica vazio.</li>
  <li>O <b>Open Library</b> é mantido por voluntários. Livros brasileiros recentes ou de editoras pequenas podem não estar lá, ou estar sem capa, sem ano ou repetidos, com dados um pouco diferentes em cada cópia. Muitas traduções faltam: quando o título não é achado, a revisão mostra os outros livros do autor, onde a obra pode estar com o título original, ou nem estar. Nesse caso, marque <b>Nenhum destes</b> e renomeie à mão.</li>
  <li>A <b>BrasilAPI</b> só ajuda livros com ISBN, quase sempre de editoras brasileiras. Ela costuma vir sem capa e sem ano. O ano da primeira publicação e a capa vêm do Open Library quando ele tem a obra; se não tiver, <code>{ano}</code> fica vazio e não há capa. Entre os autores podem vir o tradutor ou o ilustrador: quando o arquivo diz quem é o autor, só ele fica.</li>
  <li>Às vezes o <b>ISBN gravado no arquivo é de outro livro</b> (o de uma propaganda no fim, ou o da caixa de uma coleção), ou o catálogo tem um erro. Quando o título e o autor achados não batem com os do arquivo, o livro não é escolhido sozinho: vai para a revisão.</li>
  <li><code>{ano}</code> é o da primeira publicação da obra: um livro lançado em 1937 fica com 1937, mesmo numa edição de 2019. Quando o catálogo dá um ano posterior ao da edição gravada no arquivo, o que é erro dele, <code>{ano}</code> fica vazio. O ano da sua edição (<code>{ano da edição}</code>) só é conhecido quando o livro é achado pelo ISBN.</li>
  <li>A <b>série</b> e o <b>volume</b> vêm só dos dados do arquivo (do EPUB ou do FB2) ou do número no nome; o Open Library quase nunca os informa.</li>
  <li>Quadrinhos (CBZ, CBR) e revistas não são tratados por esta regra.</li>
  <li>Os dados só são <b>gravados dentro de EPUB</b>. MOBI, AZW3, PDF e FB2 ficam como estão (o <code>metadata.opf</code> ao lado serve para eles). Um EPUB danificado não é alterado e aparece na lista de erros. Um EPUB com DRM também é pulado, porque o leitor poderia recusar o livro alterado.</li>
</ul>
<p class="tip">Ao Open Library vão só o título, o autor e o ISBN procurados, e à BrasilAPI só o ISBN, nunca o nome completo do arquivo, as pastas ou o conteúdo do livro.</p>`,
  },
  {
    id: "r-comic",
    title: "Quadrinho",
    sub: true,
    html: `
<p>Dá a cada quadrinho um nome com a série e o número. Funciona de dois jeitos: <b>sem internet</b>, com o que o próprio arquivo diz, e, se você quiser, <b>buscando a série num catálogo</b> para completar nome, ano, editora e título de cada edição.</p>
${ex("Nome_da_Serie_012_(2019)_(Digital)_(grupo).cbz", "Nome da Série #012 (2019).cbz")}
<div class="notice">
<p><b>Antes de usar: o nome dos arquivos importa</b></p>
<p>Sem internet, esta regra só sabe o que cada arquivo diz de si: o <code>ComicInfo.xml</code>, quando existe, e o nome. Ela entende os jeitos mais comuns de escrever série, volume e número, mas não adivinha o que o nome não diz. Uma abreviação (<code>NmSr 12</code>) ou uma história numerada sem o nome da série (<code>07 - Nome da História</code>) não têm como virar <code>Nome da Série #007</code>.</p>
<p>Para um bom resultado, cada arquivo precisa ter pelo menos:</p>
<ul>
  <li>o <b>nome da série</b> por extenso, escrito sempre do mesmo jeito;</li>
  <li>o <b>número</b> da edição (<code>#12</code>, <code>012</code>) e, se a série teve mais de uma fase, o <b>volume</b> (<code>Vol. 2</code>);</li>
  <li>o título ou o ano, se quiser mantê-los, depois de " - " ou entre parênteses.</li>
</ul>
<p>Nomes assim funcionam bem: <code>Nome da Série #012</code>, <code>Nome da Série Vol. 2 #005</code>, <code>Nome da Série - Nome do Álbum (1990)</code>.</p>
<p>Se os seus arquivos estão longe disso, ponha acima desta regra, na lista, as regras que acertam o nome: <b>Substituir texto</b> para trocar abreviações pelo nome completo, <b>Reorganizar partes</b> ou <b>Expressão regular</b> para mudar a ordem, <b>Remover trecho</b> para tirar sobras. A regra <b>Quadrinho</b> e a busca leem o nome como ele sai dessas regras, e a coluna <b>Novo nome</b> mostra o resultado antes de renomear.</p>
<p>Dois atalhos: para uma pasta com uma série só, a opção <b>A série é o nome da pasta</b> dispensa outras regras; e, buscando no catálogo, o campo <b>Série a buscar</b> acha a série certa mesmo com um nome abreviado.</p>
</div>
<h4>Sem internet</h4>
<ul>
  <li>Em <b>CBZ, CBR, CB7 e CBT</b>, o programa lê o <code>ComicInfo.xml</code> de dentro do arquivo, quando existe: série, número, volume, título, ano, editora, roteiro e desenho. É o dado mais confiável, com uma exceção: se a série do <code>ComicInfo.xml</code> for outra que a do nome do arquivo (o original em inglês de uma edição traduzida, por exemplo), valem a série e o título do nome, que é como você chamou o arquivo.</li>
  <li>Sem ele (e sempre em <b>PDF e EPUB</b>), usa o nome do arquivo. Entende número de edição (<code>Série 012</code>, <code>#12</code>, <code>nº 12</code>, <code>Edição 12</code>, <code>#12.1</code>), volume ou tomo (<code>v02</code>, <code>Vol. 3</code>, <code>Vol. III</code>, <code>(Vol.03)</code>, <code>Tomo 5</code>, <code>T05</code>, <code>Livro 3</code>), capítulo de mangá (<code>Cap. 21</code>, <code>Ch. 21</code>, <code>c045</code>), o ano entre parênteses e o total (<code>(de 6)</code>, <code>#2 de 6</code>). O que vem entre o volume e o número é da série (<code>Série Vol. 2 Anual #01</code> é o anual, não a edição 1). Um número seguido de palavra faz parte do nome (<code>Nome com 714 Palavras</code>, <code>10 Anos</code>), e um número no começo, seguido de traço, é o da edição (<code>19 - Nome da História</code>). Marcas como <code>(Digital)</code>, <code>(c2c)</code> e nomes de grupos entre colchetes ou parênteses são ignorados.</li>
  <li>Um arquivo só com o número (<code>012.cbz</code>) pega a série da pasta onde está; uma pasta <code>Nome da Série (2012)</code> também dá o ano da série, e uma pasta <code>Vol. 3</code>, o volume.</li>
  <li>Num nome que começa pelo número (<code>19 - Nome da História</code>), o número é o do começo: números no meio do título não contam.</li>
  <li><b>A série é o nome da pasta</b>: para uma pasta de uma série só, com arquivos que trazem o número e o título da história (<code>Nome da Série (Completo)/01 - Nome da História</code>). Cada arquivo fica com a série da pasta (sem o ano e o que está entre parênteses), e o que o nome trazia vira o título: <code>Nome da Série #001 - Nome da História</code>. Se o nome só repete a série, ou uma parte dela (<code>SÉRIE v01</code> numa pasta com o nome original e o traduzido), não há título. Fica desligada, porque numa pasta que não é de uma série (<code>Downloads</code>) o nome dela viraria a série de todos.</li>
  <li>Um volume único ou encadernado sem número fica com a série e o ano (<code>Nome do Álbum (2024)</code>), sem as marcas do nome original.</li>
  <li>Alguns programas gravam letras acentuadas como o código delas entre sublinhados (<code>M_244_nica</code> para <i>Mônica</i>); o programa as devolve.</li>
  <li>Arquivos em que não dá para achar nem a série ficam como estão.</li>
</ul>
<h4>Buscando num catálogo</h4>
<p>Em <b>Onde buscar</b>, escolha o catálogo:</p>
<ul>
  <li><b>Grand Comics Database</b> (comics.org): feito por voluntários, com edições de muitos países, inclusive brasileiras. Dá o nome da série como foi publicada, o ano, a editora e os números das edições. Funciona melhor com uma <b>conta gratuita</b> do comics.org, posta em <b>Opções → Grand Comics Database</b>. Sem conta, o GCD aceita só umas 30 consultas por hora, e pode deixar de aceitar; na primeira busca sem conta, o programa avisa.</li>
  <li><b>MangaDex</b> (mangadex.org): mangás, sem conta nem chave. Dá o nome e o ano da série (no seu idioma, quando há, ou em inglês); o volume e o capítulo ficam os do arquivo.</li>
  <li><b>Comic Vine</b> (comicvine.gamespot.com): forte em quadrinhos americanos, com o título de cada edição. Pede uma chave gratuita, colada em <b>Opções</b>.</li>
</ul>
<ol>
  <li>Clique em <b>Buscar a série</b>. Os arquivos são agrupados pela série (ou use <b>Série a buscar</b> para todos de uma vez). Quando só uma série tem aquele nome e o ano dos arquivos (ou é a única achada) e edições suficientes para os números deles, ela é escolhida sozinha; sem o ano, o nome não basta (uma tradução de uma série nova pode ter o nome de uma antiga), e você escolhe; senão, você escolhe entre as publicações, que mostram país, idioma, ano de início e número de edições. Se o nome completo não achar nada (<i>Série: História</i>), a busca tenta só a série.</li>
  <li>Na janela de escolha dá para buscar de novo com outro nome e estreitar a lista com o <b>Ano</b> em que a série começou e a <b>Editora</b> (parte do nome basta; o MangaDex não tem editora). As séries com menos edições que o número dos arquivos vão para o fim. O GCD compara o nome letra por letra: maiúsculas e acentos não contam, mas a pontuação sim, por isso o programa busca o nome como foi digitado e também sem a pontuação (<i>Série: Nome</i> acha também <i>Série Nome</i>). Com o ano, o GCD é consultado pela palavra mais longa do nome, e aí a pontuação não atrapalha: <i>Série Nome</i> acha também <i>Série: Nome</i>. Na editora, vale o nome curto que o catálogo usa (<i>Titan</i> é achada com <i>Titan Comics</i>), e dá para pôr mais de uma, separadas por <code>;</code>. No GCD, a editora custa uma consulta a mais para cada editora diferente encontrada (até 12, as mais frequentes primeiro): junte o ano para serem poucas, principalmente sem conta.</li>
  <li>Cada arquivo é casado com a edição pelo número (ou pelo volume, num mangá numerado por volumes; um volume único, com a única edição da série). Os que não casam ficam com o nome da série e o número deles mesmos, na mesma pasta dos outros, e abrem a revisão, com a lista de edições da série, para escolher a edição exata. <b>Nenhuma destas</b> devolve o nome que o arquivo diz.</li>
</ol>
<ul>
  <li><b>Modelo pronto</b>: <b>Padrão do Renome</b> (que serve também para o Komga, que trata cada pasta como uma série) ou <b>Kavita</b> (sem o título depois de " - ", que o Kavita não lê). Os dois preenchem o modelo e as pastas e ligam as pastas de série, porque os dois servidores querem cada série na sua pasta. Depois dá para mudar à vontade.</li>
  <li><b>Modelo</b>: <code>{série}</code>, <code>{ano da série}</code>, <code>{número}</code> (três dígitos: <code>012</code>), <code>{volume}</code> (dois), <code>{capítulo}</code> (três), <code>{título}</code> (da edição), <code>{ano}</code> (da edição), <code>{total}</code>, <code>{editora}</code>, <code>{roteiro}</code>, <code>{arte}</code> e <code>{id}</code> (do catálogo). O padrão é <code>{série} v{volume} #{número} c{capítulo} - {título} ({ano})</code>: o que falta sai junto com a marca grudada nele (<code>v</code>, <code>#</code>, <code>c</code>), terminada em ponto (<code>Vol. </code>) ou com o traço antes. Assim, uma edição americana vira <code>Série #012 (2019)</code>, um volume de mangá, <code>Série v03</code>, e um encadernado com título, <code>Série v01 - Título (2018)</code>.</li>
  <li><b>Mais variáveis</b>, do <code>ComicInfo.xml</code>: <code>{arco}</code>, <code>{selo}</code>, <code>{formato}</code>, <code>{gênero}</code>, <code>{arte-final}</code>, <code>{colorista}</code>, <code>{letrista}</code>, <code>{capista}</code>, <code>{editor}</code>, <code>{tradutor}</code>, <code>{páginas}</code>, <code>{faixa etária}</code>, <code>{idioma}</code>, <code>{série alternativa}</code>, <code>{número alternativo}</code>, <code>{gtin}</code>, e a data de capa (a do catálogo, quando a edição é achada nele). Datas: <code>{data}</code> (<code>2010-05-03</code>), <code>{mês}</code> (<code>05</code>), <code>{dia}</code> e o nome do mês na língua da variável: <code>{nome do mês}</code> dá <code>maio</code>, <code>{month name}</code> dá <code>May</code> e <code>{nombre del mes}</code> dá <code>mayo</code>. <code>{scan}</code> é quem digitalizou: a do <code>ComicInfo.xml</code>, senão a última nota entre parênteses no nome que não é ano nem palavra conhecida (<code>(Grupo)</code>), ou o que vem depois de <code>--</code>. O que vem depois de <code>--</code> ou <code>__</code> nunca entra no nome da série, e <code>c2c</code> e <code>FCBD</code> são ignorados.</li>
  <li><b>Outros formatos de dados</b>: sem <code>ComicInfo.xml</code>, o <code>MetronInfo.xml</code> de dentro do arquivo é lido (série, número, volume, histórias, data de capa, editora e selo, criadores por função, arco, páginas, classificação, idioma, GTIN e o ID do Comic Vine); sem os dois, os dados que alguns programas guardam no comentário do zip (ComicBookInfo).</li>
  <li><b>Trechos opcionais</b>: um trecho entre <code>&lt; &gt;</code> só aparece quando todas as variáveis dentro dele têm valor (os sinais <code>&lt; &gt;</code> não vão para o nome). <code>{série} #{número}&lt; - {roteiro}&gt;</code> só põe o roteirista quando ele é conhecido. Vale também no modelo das pastas.</li>
  <li><b>Reserva</b>: <code>{a|b}</code> põe a primeira das variáveis que tiver valor. <code>{título|ano}</code> dá o título, ou o ano quando não há título. Pode ter mais de duas (<code>{a|b|c}</code>) e funciona dentro de um trecho opcional.</li>
  <li><b>Algarismos</b>: <code>{variável:N}</code> escreve os números com N algarismos. <code>{a:3}</code> dá <code>005</code> em vez de <code>05</code>, e <code>{a:1}</code> dá <code>5</code>, sem zeros na frente. Casas decimais (<code>12.1</code>) e o disco antes da faixa (<code>1-05</code>) ficam como estão; texto sem número não muda.</li>
  <li><b>Organizar em pastas de série</b>: cada quadrinho vai para <code>{série} ({ano da série})</code> (o modelo das pastas pode ser trocado; cada <code>/</code> é um nível). Um quadrinho que já está numa pasta com o nome da série fica onde está, mesmo que a pasta tenha outro formato (<code>Nome da Série Vol. 2 (1988-1998)</code>, <code>Nome da Série/Vol. 1</code>, <code>Nome da Série</code> para um especial <code>Nome da Série - Especial</code>, ou uma grafia um pouco diferente). Uma pasta em que todos os quadrinhos são de uma série só conta como a pasta da série, qualquer que seja o nome (<code>Meus quadrinhos</code>): a pasta nova fica ao lado dela, não dentro, e o que sobrar nela segue a regra das pastas com sobras (veja Pré-visualização e conflitos). Os quadrinhos da lista são organizados juntos, como uma biblioteca:<ul><li>cada série fica com uma grafia só, a mais comum na lista (maiúsculas, acentos e uma letra trocada numa palavra, como em <code>Apocalipse</code> e <code>Apocalypse</code>, contam como a mesma série), no nome da pasta e no dos arquivos;</li><li>um especial ou volume único vai para a pasta da série cujo nome começa o dele (<code>Nome da Série - Especial</code> em <code>Nome da Série</code>; havendo mais de uma, a de nome mais longo);</li><li>volumes únicos com a mesma parte antes do " - " (<code>Nome da Série - Álbum Um</code>, <code>Nome da Série - Álbum Dois</code>) vão juntos para uma pasta com essa parte;</li><li>os outros ficam cada um na pasta da sua série, mesmo com um arquivo só.</li></ul>Desfazer devolve tudo ao lugar.</li>
</ul>
<h4>Gravar os dados no arquivo</h4>
<p>Com <b>Gravar os dados no arquivo (ComicInfo.xml)</b>, depois de renomear, cada CBZ casado com uma edição do catálogo recebe dentro dele um <code>ComicInfo.xml</code> com série, número, título, total de edições, volume, data de capa, roteiro, desenho, editora, o endereço da edição no catálogo e o que mais o arquivo já informava (arco, selo, colorista…). É o arquivo que o Komga, o Kavita e os leitores de quadrinhos leem. Se o CBZ já tinha um, só os campos que mudam são trocados: sinopse, lista de páginas e o resto ficam. As páginas são copiadas como estão, sem perder qualidade. Quadrinhos que já têm o nome certo também recebem os dados (eles contam no botão <b>Renomear</b>), e os que já têm os mesmos dados são pulados. Os achados no MangaDex ficam marcados como mangá.</p>
<p>A gravação é feita numa cópia, que só toma o lugar do arquivo no fim; se o programa for fechado no meio, o que já foi gravado vai para o <b>Histórico</b>. Desfazer devolve o <code>ComicInfo.xml</code> antigo, ou tira o que foi posto. Como o conteúdo muda, um torrent não reconhece mais esses arquivos.</p>
<h4>Converter para CBZ</h4>
<p>Com <b>Converter CBR, CB7 e CBT para CBZ</b>, cada quadrinho em RAR, 7z ou TAR passa a ser um CBZ, o formato que todo leitor abre. A extensão muda já na prévia (<code>Nome.cbr</code> → <code>Nome.cbz</code>), e os <code>.cbz</code> que na verdade são RAR ou 7z também são convertidos, mesmo sem mudar de nome (eles contam no botão <b>Renomear</b>). As páginas são copiadas como estão, na mesma ordem e sem perder qualidade. Com <b>Gravar os dados no arquivo</b> também ligada, o <code>ComicInfo.xml</code> já vai no CBZ novo; sem ela, o que o original trazia vai igual.</p>
<p>O CBZ é escrito numa cópia ao lado e só toma o lugar do original quando está completo e conferido. Em <b>O arquivo original</b> você escolhe o que acontece com ele. <b>Manter ao lado</b> deixa o original na mesma pasta, com o nome novo e a extensão dele (<code>Nome.cbr</code>); desfazer o põe de volta e apaga o CBZ. <b>Excluir</b> manda o original para a Lixeira do sistema, de onde dá para recuperá-lo; desfazer só devolve o nome antigo ao CBZ, que continua CBZ.</p>
<h4>Edições que faltam</h4>
<p>Com <b>Mostrar as edições que faltam</b> (ligada por padrão), depois da busca a lista mostra linhas apagadas para as edições já publicadas da série que nenhum arquivo da lista tem, como <code>#7 · Título</code>, com a data de capa. Elas ficam no lugar da edição, entre as vizinhas, e a barra de baixo mostra quantas faltam. São só um aviso: não podem ser selecionadas e não entram em contagens nem em renomeação. <b>Clique nesse total</b> para salvar as edições que faltam num arquivo CSV (série, número, título e data de capa); a janela abre na pasta da série. Só entram as séries de que a lista tem algum arquivo casado com uma edição.</p>
<h4>Limitações</h4>
<ul>
  <li>Os <b>CBR</b> (RAR) são lidos com a libarchive, uma biblioteca de código aberto embutida no programa. Ela abre quase todos, mas alguns RAR antigos dão erro (o programa oficial do RAR abre): esses ficam como estão e aparecem na lista de erros. RAR com senha e RAR divididos em partes (<code>.part1.rar</code>) não são convertidos. Arquivos RAR não podem ser escritos: para gravar os dados num CBR, é preciso convertê-lo.</li>
  <li>Nomes de arquivo muito fora do comum podem ser lidos errado (um número que faz parte do nome da série, por exemplo). Confira a coluna <b>Novo nome</b> antes de renomear.</li>
  <li>Os catálogos têm lacunas: edições brasileiras e de outros países podem faltar no Comic Vine, e o GCD, feito por voluntários, pode estar incompleto. Quando não houver a série certa, pule-a: os arquivos ficam com o nome que eles mesmos dizem. A busca nem sempre acha a série pelo nome traduzido; no seletor, dá para buscar de novo com outro nome (o original, por exemplo).</li>
  <li>No GCD, a revisão mostra só os números das edições, sem título nem capa: trazê-los custaria uma consulta por edição. Uma série com o mesmo nome em outro idioma (a edição francesa, quando há uma em português) não é escolhida sozinha.</li>
  <li>O GCD e o Comic Vine contam <b>edições e volumes</b>, não capítulos: capítulos soltos de mangá não são buscados e ficam com o nome lido do arquivo. O MangaDex só dá a série.</li>
  <li><b>Limites de consultas</b>: o GCD aceita umas 25 por minuto com conta (umas 30 por hora sem conta); o Comic Vine, umas 200 por hora, só para uso não comercial. O programa gasta poucas por série e guarda as respostas enquanto estiver aberto; se chegar ao limite, espere um pouco e busque de novo.</li>
  <li>As <b>edições que faltam</b> seguem a lista do catálogo: anuais, especiais e edições de numeração estranha (<code>#1.5</code>, <code>#0</code>) aparecem como faltando se você não as tiver, e uma série com centenas de edições, de que você tem poucas, enche a lista. Desligue a opção nesse caso. No GCD, sem data de capa.</li>
  <li>O <b>MangaDex</b> não tem a lista de volumes de um mangá (só dos capítulos que os leitores enviaram, e os de mangás licenciados costumam ter sido tirados). Por isso, nele, as que faltam são só as que faltam <b>entre os seus arquivos</b>, do 1 até o maior número que você tem (por volume, quando todos os arquivos têm um; senão pelo número ou pelo capítulo): o que vem depois do último não aparece.</li>
  <li>No <b>GCD</b>, cada capa variante é cadastrada como uma edição à parte com o mesmo número; o programa fica só com uma por número (a capa comum, quando há), na contagem, na revisão e nas edições que faltam.</li>
  <li>Os dados só são <b>gravados em CBZ</b> (que seja ZIP por dentro). CBR, CB7 e CBT recebem os dados só se forem convertidos; PDF e EPUB ficam como estão. Um arquivo danificado não é alterado e aparece na lista de erros. Sem as opções ligadas, o arquivo nunca é alterado.</li>
  <li>A <b>conversão</b> lê e escreve cada arquivo inteiro: numa coleção grande leva tempo e precisa de espaço livre para uma cópia de cada vez. O CBZ fica mais ou menos do tamanho do original (às vezes um pouco maior, porque as páginas não são comprimidas de novo). Com <b>Manter ao lado</b>, os dois ficam na pasta, e o Komga e o Kavita mostram os dois até você apagar os originais. Com <b>Excluir</b>, desfazer não traz o original de volta: ele fica na Lixeira, até você esvaziá-la. Na versão Flatpak o original não vai para a Lixeira: fica ao lado.</li>
  <li>Só se grava o que o catálogo (ou o próprio arquivo) informa: no GCD, os números das edições vêm sem título, e no MangaDex só há a série. Um <code>MetronInfo.xml</code> que o arquivo tenha não é atualizado.</li>
</ul>
<p class="tip">Aos catálogos vão só o nome da série procurada e os números das séries e edições, nunca o nome completo do arquivo, as pastas ou o conteúdo. O login do GCD vai só ao comics.org.</p>`,
  },
  {
    id: "preview",
    title: "Pré-visualização e conflitos",
    html: `
<p>A coluna <b>Novo nome</b> mostra o resultado das regras na hora. Nas linhas que mudam:</p>
<ul>
  <li>no <b>Nome atual</b>, o que vai sair aparece <span class="del">em vermelho, riscado</span>;</li>
  <li>no <b>Novo nome</b>, o que entra aparece <span class="ins">em azul</span>;</li>
  <li>o que só mudou de lugar (o autor que passou para a frente do título, por exemplo) aparece <span class="mov">sublinhado com pontinhos</span>, nos dois lados.</li>
  <li>um arquivo que vai mudar de pasta ganha um ícone de pasta no novo nome, e a coluna <b>Pasta</b> mostra, em azul, a pasta completa para onde ele vai (<code>→ ~/Música/Artista/Álbum (2019)</code>), no mesmo formato da pasta atual dos outros arquivos.</li>
  <li>com uma regra de busca ligada (filme, série, música, livro, quadrinho), um arquivo que ela não trata (um <code>.zip</code> na pasta da série, por exemplo) ganha a etiqueta cinza <b>fora da regra</b>; passe o mouse para ver o motivo. A linha continua normal: dá para renomeá-lo à mão, mas a busca não vai mexer nele.</li>
</ul>
<p>Na primeira coluna, ● indica que o arquivo será renomeado, <span class="bad">!</span> vermelho indica um conflito e <span class="warn">!</span> amarelo indica um aviso. Passe o mouse para ver o motivo.</p>
<h4>Editar à mão</h4>
<p>Clique duas vezes no novo nome (ou selecione um arquivo e aperte <kbd>F2</kbd>) para escrever o nome que quiser. <kbd>Enter</kbd> confirma e <kbd>Esc</kbd> cancela. Um nome editado à mão ignora as regras, mas a pasta para onde elas levam o arquivo (a da série, a do filme) continua valendo; ↺ volta a usar as regras.</p>
<h4>Pastas antigas que ficam com arquivos</h4>
<p>Quando as regras dão um nome novo à pasta de uma série, de um filme, de um álbum, de um autor ou de uma série de quadrinhos, a pasta nova é criada <b>ao lado</b> da antiga, e os arquivos passam para ela. A pasta antiga que ficar vazia é apagada. Se ainda ficarem arquivos nela (extras, imagens, um arquivo em conflito, um arquivo fora da lista), ela não é apagada: passa a se chamar como a nova, com <code>(arquivos adicionais)</code> no fim, e as duas aparecem juntas. Por exemplo, <code>Nome do Filme.1999.1080p.BluRay</code> com um <code>sample.mkv</code> vira <code>Nome do Filme (1999) [tmdbid-12345] (arquivos adicionais)</code>, ao lado de <code>Nome do Filme (1999) [tmdbid-12345]</code>.</p>
<ul>
  <li>Antes de renomear, a barra de baixo mostra, em amarelo, <b>pasta(s) com sobras</b> (passe o mouse para ver quais), e a confirmação diz qual pasta e quantos arquivos ficam nela, mesmo com a confirmação desligada.</li>
  <li><b>Desfazer</b> devolve os arquivos e o nome antigo da pasta.</li>
  <li>A pasta pessoal, Downloads, Documentos, Vídeos, Música, Imagens, a Área de trabalho e o topo de um disco nunca são renomeados.</li>
  <li>Uma pasta que vira duas (<code>Título do Livro</code> → <code>Autor/Título do Livro</code>) fica com o nome que tem: não há uma pasta nova só ao lado dela. O mesmo vale para uma pasta de onde os arquivos vão para lugares diferentes (uma pasta com músicas de vários artistas).</li>
  <li>Um filme só ganha a pasta nova ao lado quando a pasta onde está tem o nome dele (com ou sem marcas como 1080p) ou quando ela só tem o filme e o que o acompanha: legendas, imagens, <code>.nfo</code> e <code>.txt</code>. Uma pasta com outro vídeo, um documento ou uma planilha não conta: a pasta do filme é criada dentro dela.</li>
</ul>
<h4>Conflitos</h4>
<p>Arquivos com conflito não são renomeados. Os motivos são:</p>
<ul>
  <li>nome vazio, ou contendo <code>/</code>;</li>
  <li>nome com mais de 255 bytes (no Windows, mais de 255 caracteres);</li>
  <li>dois arquivos ficariam com o mesmo nome na mesma pasta;</li>
  <li>já existe um arquivo com esse nome fora da lista (nada é sobrescrito);</li>
  <li>no Windows: os caracteres <code>\\ : * ? " &lt; &gt; |</code>, ou ponto ou espaço no fim do nome.</li>
</ul>
<p>Em outros sistemas, esses caracteres do Windows só geram um aviso amarelo, desligável nas Opções. No Windows, no macOS e em discos que não diferenciam maiúsculas (exFAT, FAT, NTFS, como pendrives), nomes que só diferem em maiúsculas contam como iguais.</p>
<p><b>Exportar lista</b>, no rodapé, salva a lista como está na tela (pasta, nome atual, novo nome, pasta para onde vai, situação e problema) num arquivo CSV, que abre em qualquer planilha. Serve para conferir com calma um lote grande antes de renomear.</p>
<p>Quando há o que falta (episódios, edições, faixas, filmes ou volumes), o rodapé mostra o total, com uma caixa ao lado: <b>desmarcada</b>, as linhas apagadas somem da lista, para ver só os seus arquivos; <b>marcada</b>, elas voltam. Nada é buscado de novo, e o total e a exportação continuam iguais. A escolha fica salva.</p>
`,
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
<p>A busca é feita em etapas para ser rápida: só arquivos do mesmo tamanho são comparados; depois, só o começo e o fim; o arquivo inteiro só é lido quando tudo isso coincide. Com arquivos grandes, o rodapé mostra o progresso. Arquivos vazios, pastas e atalhos (links simbólicos) ficam de fora.</p>
<h4>Músicas repetidas</h4>
<p>Além das cópias idênticas, o programa acha a <b>mesma música em arquivos diferentes</b>: um MP3 e um FLAC da mesma faixa, ou uma música no seu álbum e de novo numa coletânea. Elas ganham uma etiqueta com contorno, <b>♪</b> e o número do grupo (passe o mouse para ver quais são as outras), e o rodapé mostra <b>N música(s) repetida(s)</b>: clique para ver só elas, agrupadas. Nada é apagado nem alterado.</p>
<ul>
  <li>Contam como a mesma música: mesmo <b>título</b> e mesmo <b>artista</b> nas tags, sem diferença de maiúsculas, acentos e pontuação, e sem o que está entre parênteses ou colchetes (<code>(Remaster 2011)</code>); participações (<code>feat.</code>) não contam. A <b>duração</b> precisa ser a mesma, com até 3 segundos de diferença.</li>
  <li>Cópias idênticas de um mesmo arquivo já aparecem como cópias; só entram aqui quando há também outro arquivo da mesma música.</li>
</ul>
<p><b>Limitações:</b> só valem as tags, não o som. Músicas sem título, sem artista ou sem duração ficam de fora; tags erradas geram falsos alarmes ou deixam repetidas passarem. Uma versão ao vivo ou remixada com o mesmo título e quase a mesma duração conta como a mesma música, e a mesma gravação com títulos escritos de jeitos diferentes (<code>Pt. 1</code> e <code>Part One</code>) não é reconhecida. Na primeira vez, as tags de cada música da lista são lidas, o que demora um pouco em pastas grandes.</p>`,
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
<h4>Enviar para outra pasta</h4>
<p>Ao lado do botão Renomear, o seletor diz para onde vão os arquivos: <b>Na mesma pasta</b> (o normal: só renomeia), <b>Mover para outra pasta…</b> ou <b>Copiar para outra pasta…</b>. Serve para organizar uma pasta de downloads direto na sua biblioteca, inclusive em outro disco. A escolha não fica salva e volta para "Na mesma pasta" depois de cada envio: a lista passa a mostrar os arquivos no lugar novo, e enviá-los de novo os mandaria outra vez.</p>
<ul>
  <li>As pastas que as regras criam (artista e álbum, filme, série e temporada, autor, série de quadrinhos) ficam dentro da pasta escolhida: <code>Destino/Artista/Álbum (1999)/01 - Música.flac</code>.</li>
  <li>Os outros arquivos mantêm as subpastas a partir da pasta que você adicionou: com <code>Fotos</code> adicionada, <code>Fotos/2024/Praia/x.jpg</code> vai para <code>Destino/2024/Praia/x.jpg</code>. Arquivos adicionados um a um vão direto para o destino.</li>
  <li>Todos os arquivos visíveis vão, mesmo os que mantêm o nome. A coluna Pasta mostra onde cada um vai ficar.</li>
  <li><b>Copiar</b>: os originais ficam exatamente como estão, e as cópias recebem os nomes novos. Capas, <code>.nfo</code>, playlists e tags gravadas vão para as cópias. Listas <code>.cue</code>/<code>.m3u</code> copiadas junto passam a citar as cópias; as originais não mudam. A data de modificação de cada arquivo é mantida na cópia.</li>
  <li>As pastas que ficarem vazias na origem são apagadas, mas só as que estão dentro da pasta que você adicionou: ela mesma, e as de fora dela, ficam.</li>
  <li><b>Mover</b> para outro disco: o arquivo é copiado e só é apagado do disco de origem depois que a cópia está completa e conferida.</li>
  <li>Durante a cópia aparece o progresso e o botão <b>Parar</b>: o que já foi copiado fica (e pode ser desfeito), e o resto fica como estava.</li>
  <li>Cada cópia é feita com um nome provisório e só recebe o nome final quando está inteira; se o programa fechar no meio, a cópia pela metade é apagada ao abrir de novo. Nada é sobrescrito: um nome que já existe no destino aparece como conflito.</li>
  <li><b>Desfazer</b> uma cópia apaga as cópias criadas, mas só as que continuam como o Renome as deixou. Uma cópia que você editou depois fica, e o aviso diz quais. Desfazer um movimento traz os arquivos de volta, mesmo de outro disco.</li>
</ul>
<p><b>Limitações</b>: o espaço livre no destino não é conferido antes; se acabar, os arquivos que não couberem falham (sem deixar nada pela metade) e aparecem na lista de erros. Copiar muitos arquivos grandes leva o tempo do disco: o Renome não acelera a cópia. Pastas adicionadas como itens são copiadas ou movidas inteiras.</p>
<h4>Histórico</h4>
<p>Cada renomeação fica registrada no <b>Histórico</b>, mesmo depois de fechar o programa. Lá você pode:</p>
<ul>
  <li><b>Desfazer</b>: volta os nomes anteriores.</li>
  <li><b>Tentar de novo</b>: aparece quando parte do desfazer falhou (por exemplo, um arquivo foi movido depois). Devolva o arquivo ao lugar e tente de novo.</li>
  <li><b>✕</b>: exclui a entrada do histórico. Os arquivos não mudam, mas aquela renomeação não poderá mais ser desfeita.</li>
  <li><b>Limpar histórico</b>: exclui todas as entradas de uma vez, com o mesmo efeito.</li>
</ul>
<p class="tip"><kbd>Ctrl</kbd>+<kbd>Z</kbd> desfaz alterações na <i>lista</i> (adicionar, remover, mover, editar nome). Para desfazer uma renomeação já feita, use o Histórico.</p>
<p>Se o programa for fechado ou o computador desligar no meio de uma renomeação (ou de um desfazer), na próxima vez que o Renome abrir ele termina o serviço com segurança: os arquivos que ainda estavam com um nome temporário voltam ao nome de antes, os que já tinham sido renomeados entram no <b>Histórico</b>, para você desfazer se quiser, e os que um desfazer já tinha devolvido saem dele. Nenhum arquivo se perde.</p>
`,
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
  <tr><td><kbd>↑</kbd> <kbd>↓</kbd> <kbd>Home</kbd> <kbd>End</kbd></td><td>Andar pela lista de arquivos e selecionar (chegue nela com <kbd>Tab</kbd> ou um clique); com <kbd>Shift</kbd>, estende a seleção</td></tr>
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
  <li><b>Avisar sobre nomes que o Windows não aceita</b>: caracteres como <code>:</code> e <code>?</code>, ponto ou espaço no fim, e nomes reservados como <code>CON</code> e <code>NUL</code>. Útil para arquivos que vão para o Windows. Num pendrive ou disco formatado para Windows (FAT, exFAT), esses nomes não são aceitos de jeito nenhum: lá eles são erro, com ou sem esta opção.</li>
  <li><b>Buscas online: guardar as respostas por</b> 1 dia, 7 dias (o padrão), 30 dias, 6 meses, ou não guardar. As respostas do TMDB, MusicBrainz, AcoustID, Discogs, Open Library, Grand Comics Database, Comic Vine e MangaDex ficam guardadas no disco; buscar de novo os mesmos arquivos fica instantâneo e não gasta os limites dos serviços (o GCD sem conta aceita só umas 30 consultas por hora). O TMDB só permite guardar por até 6 meses, e as do Discogs valem só 6 horas, qualquer que seja a escolha. As chaves e senhas não vão junto. O botão <b>Limpar</b> apaga tudo e mostra quanto ocupa. Limitação: uma série ainda em exibição, uma série de quadrinhos em andamento ou uma correção feita no catálogo só aparecem depois que a resposta guardada vence; para ver logo, limpe.</li>
  <li><b>TMDB</b>, <b>Comic Vine</b>, <b>Grand Comics Database</b> e <b>Discogs</b>: as chaves, o token e a conta que as regras de filme, série, quadrinho e música usam; cada um tem um botão <b>Testar</b>.</li>
  <li>As chaves, o token e a senha ficam só neste computador, <b>criptografados no cofre de senhas do sistema</b> (o Chaveiro do GNOME ou a Carteira do KDE no Linux, o Gerenciador de Credenciais no Windows, as Chaves no macOS), e não no arquivo de opções. Limitação: um sistema sem cofre de senhas (algumas instalações mínimas do Linux) não tem onde criptografá-los; aí eles ficam no arquivo de opções, sem criptografia, e as Opções avisam. O cofre protege contra quem lê os arquivos do disco, não contra outro programa aberto na sua própria sessão.</li>
  <li>Quando o cofre está trancado (por exemplo, com login automático no Linux), o sistema pede a senha dele; o programa abre mesmo assim, e as chaves aparecem depois de destrancar. No macOS, como o Renome não é assinado pela Apple, o sistema pode perguntar se ele pode usar as Chaves, de novo a cada versão nova; escolha <b>Permitir sempre</b>. Se o acesso for negado (ou o cofre não for destrancado), as chaves guardadas lá não são apagadas nem trocadas: as Opções avisam e têm um botão <b>Tentar de novo</b>.</li>
</ul>
<p>As opções ficam em <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) ou <code>~/Library/Application Support/renome</code> (macOS). O histórico fica em <code>~/.local/share/renome/history</code> no Linux e na mesma pasta das opções nos outros sistemas. As respostas guardadas das buscas ficam em <code>~/.cache/renome/answers</code> (Linux), <code>%LOCALAPPDATA%\\renome\\answers</code> (Windows) ou <code>~/Library/Caches/renome/answers</code> (macOS).</p>`,
  },
];
