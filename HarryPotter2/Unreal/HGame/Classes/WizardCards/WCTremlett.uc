//===============================================================================
// #34 Donaghan Tremlett
//===============================================================================

class  WCTremlett extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardTremlettTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Tremlettsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardTremlettBigTexture FILE=TEXTURES\menu\Folio\Cards\Tremlettbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Donaghan Tremlett";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=34
    skin(0)=Texture'HProps.skins.WizardCardTremlettTex0'
	textureBig=Texture'WizCardTremlettBigTexture'
	strDescriptionId="WizCard_0030"
}
