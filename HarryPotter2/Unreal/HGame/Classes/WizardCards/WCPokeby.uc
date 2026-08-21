//===============================================================================
//  #5 Gulliver Pokeby
//===============================================================================

class  WCPokeby extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardPokebyTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Pokebysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPokebyBigTexture FILE=TEXTURES\menu\Folio\Cards\Pokebybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Gulliver Pokeby";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=5
    skin(0)=Texture'HProps.skins.WizardCardPokebyTex0'
	textureBig=Texture'WizCardPokebyBigTexture'
	strDescriptionId="WizCard_0043"
}
