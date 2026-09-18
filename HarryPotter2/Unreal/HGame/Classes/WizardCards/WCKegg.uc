//===============================================================================
// #84 Roland Kegg
//===============================================================================

class  WCKegg extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardKeggTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Keggsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardKeggBigTexture FILE=TEXTURES\menu\Folio\Cards\Keggbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Roland Kegg";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=84
    skin(0)=Texture'HProps.skins.WizardCardKeggTex0'
	textureBig=Texture'WizCardKeggBigTexture'
	strDescriptionId="WizCard_0090"
}
