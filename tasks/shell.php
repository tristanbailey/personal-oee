<?php

/**
 * Interactive PsySH shell with the Jigsaw site data loaded (like `artisan tinker`).
 *
 *   composer shell            # or: php tasks/shell.php [env]
 *
 * Gives you:
 *   $app      the Jigsaw container (config in $app->config)
 *   $site     the SiteData collection (all collections, plus $site->page config)
 *   $posts, $casestudies ... each collection found in config.php, as a Collection
 *
 * Nothing is rendered or written to build_local/.
 */
chdir(dirname(__DIR__));
require getcwd().'/vendor/autoload.php';

use Illuminate\Contracts\Debug\ExceptionHandler;
use Psy\Configuration;
use Psy\Shell;
use Symfony\Component\Console\Output\OutputInterface;
use TightenCo\Jigsaw\Bootstrap\HandleExceptions;
use TightenCo\Jigsaw\Container;
use TightenCo\Jigsaw\Exceptions\Handler;
use TightenCo\Jigsaw\Loaders\DataLoader;

$app = new Container;
$app->singleton(ExceptionHandler::class, Handler::class);
$app->bootstrapWith([HandleExceptions::class]);

$env = $app['env'] = $argv[1] ?? 'local';

// BuildCommand normally fills these in before a build.
$app->buildPath = [
    'source' => $app->path('source'),
    'views' => $app->path('source'),
    'destination' => $app->path("build_{$env}"),
];
if (file_exists($envConfig = $app->path("config.{$env}.php"))) {
    $app->config = $app->config->merge(require $envConfig)->filter(fn ($item) => $item !== null);
}

$app->consoleOutput->setup(OutputInterface::VERBOSITY_QUIET);

$loader = $app->make(DataLoader::class);
$site = $loader->loadSiteData($app->config);
$site = $site->addCollectionData($loader->loadCollectionData($site, $app->buildPath['source']));

$vars = ['app' => $app, 'site' => $site];
foreach ($app->config->get('collections', []) as $name => $_) {
    $vars[$name] = $site->get($name);
}

$shell = new Shell(new Configuration(['updateCheck' => 'never']));
$shell->setScopeVariables($vars);
echo 'Jigsaw shell ('.$app['env'].'). Variables: $'.implode(', $', array_keys($vars)).PHP_EOL;
$shell->run();
